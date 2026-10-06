use std::error::Error;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tempfile::tempdir;
use tower_lsp::lsp_types::Url;
use zk::archive::Archive;

#[derive(Clone, Copy)]
enum Encoding {
    Utf8,
    Utf16,
}

impl Encoding {
    fn name(self) -> &'static str {
        match self {
            Self::Utf8 => "utf-8",
            Self::Utf16 => "utf-16",
        }
    }
}

struct LspClient {
    child: Child,
    input: Option<BufWriter<ChildStdin>>,
    messages: Receiver<Result<Value, String>>,
    notifications: Vec<Value>,
    registrations: Vec<Value>,
    next_id: u64,
}

impl LspClient {
    fn spawn(binary: &Path, archive: &Path) -> Result<Self, Box<dyn Error>> {
        let mut child = Command::new(binary)
            .arg("--archive")
            .arg(archive)
            .arg("lsp")
            .current_dir(archive.parent().ok_or("archive has no parent")?)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let input = BufWriter::new(child.stdin.take().expect("server stdin"));
        let output = child.stdout.take().expect("server stdout");
        let (sender, messages) = mpsc::channel();
        std::thread::spawn(move || {
            let mut output = BufReader::new(output);
            loop {
                match read_message(&mut output) {
                    Ok(Some(message)) => {
                        if sender.send(Ok(message)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let _ = sender.send(Err(error.to_string()));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child,
            input: Some(input),
            messages,
            notifications: Vec::new(),
            registrations: Vec::new(),
            next_id: 1,
        })
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, Box<dyn Error>> {
        let response = self.response(method, params)?;
        if let Some(error) = response.get("error") {
            return Err(format!("{method} failed: {error}").into());
        }
        let result = response.get("result").cloned().unwrap_or(Value::Null);
        if method == "workspace/executeCommand" {
            assert_eq!(result["schema_version"], 2);
            return Ok(result
                .get("data")
                .expect("schema-2 result envelope")
                .clone());
        }
        Ok(result)
    }

    fn request_error(&mut self, method: &str, params: Value) -> Result<Value, Box<dyn Error>> {
        let response = self.response(method, params)?;
        assert!(response.get("result").is_none());
        Ok(response
            .get("error")
            .expect("JSON-RPC error response")
            .clone())
    }

    fn response(&mut self, method: &str, params: Value) -> Result<Value, Box<dyn Error>> {
        let id = self.next_id;
        self.next_id += 1;
        let mut message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
        });
        if !params.is_null() {
            message["params"] = params;
        }
        self.send(message)?;
        loop {
            let message = self.receive()?;
            if self.handle_server_request(&message)? {
                continue;
            }
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                return Ok(message);
            }
            self.notifications.push(message);
        }
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), Box<dyn Error>> {
        let mut message = json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        if !params.is_null() {
            message["params"] = params;
        }
        self.send(message)
    }

    fn wait_notification(
        &mut self,
        method: &str,
        predicate: impl Fn(&Value) -> bool,
    ) -> Result<Value, Box<dyn Error>> {
        if let Some(index) = self.notifications.iter().position(|message| {
            message.get("method").and_then(Value::as_str) == Some(method)
                && predicate(&message["params"])
        }) {
            return Ok(self.notifications.remove(index));
        }
        loop {
            let message = self.receive().map_err(|error| {
                format!(
                    "waiting for {method}; queued messages: {}; receive error: {error}",
                    serde_json::to_string(&self.notifications).unwrap_or_default()
                )
            })?;
            if self.handle_server_request(&message)? {
                continue;
            }
            if message.get("method").and_then(Value::as_str) == Some(method)
                && predicate(&message["params"])
            {
                return Ok(message);
            }
            self.notifications.push(message);
        }
    }

    fn handle_server_request(&mut self, message: &Value) -> Result<bool, Box<dyn Error>> {
        let Some(id) = message.get("id").cloned() else {
            return Ok(false);
        };
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Ok(false);
        };
        if method != "client/registerCapability" {
            return Err(format!("unexpected server request: {method}").into());
        }
        self.registrations.push(message.clone());
        self.send(json!({"jsonrpc": "2.0", "id": id, "result": null}))?;
        Ok(true)
    }

    fn shutdown(mut self) -> Result<(), Box<dyn Error>> {
        assert!(self.request("shutdown", Value::Null)?.is_null());
        self.notify("exit", Value::Null)?;
        self.finish_exit(0)
    }

    fn finish_exit(mut self, expected: i32) -> Result<(), Box<dyn Error>> {
        drop(self.input.take());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait()? {
                if status.code() != Some(expected) {
                    return Err(format!(
                        "language server exited with {status}, expected {expected}"
                    )
                    .into());
                }
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("language server did not exit after shutdown".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn send(&mut self, message: Value) -> Result<(), Box<dyn Error>> {
        let body = serde_json::to_vec(&message)?;
        let input = self.input.as_mut().expect("server stdin is open");
        write!(input, "Content-Length: {}\r\n\r\n", body.len())?;
        input.write_all(&body)?;
        input.flush()?;
        Ok(())
    }

    fn receive(&self) -> Result<Value, Box<dyn Error>> {
        match self.messages.recv_timeout(Duration::from_secs(5))? {
            Ok(message) => Ok(message),
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // Also reap the server when an assertion panics or a request times out.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, Box<dyn Error + Send + Sync>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some(value) = line
            .strip_prefix("Content-Length:")
            .map(str::trim)
            .and_then(|value| value.parse::<usize>().ok())
        {
            content_length = Some(value);
        }
    }
    let length = content_length.ok_or("missing Content-Length header")?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

fn zettel(title: &str, id: &str, keywords: &str, body: &str) -> String {
    format!(
        r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= {title} <{id}>

#abstract[Live protocol test.]

#keywords({keywords})

#category.thoughts

{body}
"#
    )
}

fn position(text: &str, offset: usize, encoding: Encoding) -> Value {
    let head = &text[..offset];
    let line = head.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = head.rfind('\n').map_or(0, |index| index + 1);
    let line_head = &text[line_start..offset];
    let character = match encoding {
        Encoding::Utf8 => line_head.len(),
        Encoding::Utf16 => line_head.encode_utf16().count(),
    };
    json!({"line": line, "character": character})
}

fn write_target(path: &Path, title: &str) -> Result<(), Box<dyn Error>> {
    fs::write(
        path,
        zettel(title, "2603231411", "\"target\"", "No outgoing links."),
    )?;
    Ok(())
}

#[test]
fn protocol_over_stdio_with_utf8_positions() -> Result<(), Box<dyn Error>> {
    protocol_session(Encoding::Utf8)
}

#[test]
fn protocol_over_stdio_with_utf16_positions() -> Result<(), Box<dyn Error>> {
    protocol_session(Encoding::Utf16)
}

#[test]
fn schema_two_node_fixture_and_generic_hover_over_stdio() -> Result<(), Box<dyn Error>> {
    let temporary = tempdir()?;
    Archive::init(temporary.path())?;
    fs::write(
        temporary.path().join("zk.toml"),
        include_str!("fixtures/schema2/zk.toml"),
    )?;
    fs::write(
        temporary.path().join("zettel/2603231410.typ"),
        include_str!("fixtures/schema2/note.typ"),
    )?;
    let source = "= Caller <2603231411>\nSee @2603231410.\n";
    let source_path = temporary.path().join("zettel/2603231411.typ");
    fs::write(&source_path, source)?;
    let source_uri = Url::from_file_path(&source_path).unwrap();
    let mut client = LspClient::spawn(Path::new(env!("CARGO_BIN_EXE_zk")), temporary.path())?;
    client.request("initialize", json!({"capabilities": {}}))?;
    client.notify("initialized", json!({}))?;
    let response = client.response(
        "workspace/executeCommand",
        json!({"command": "zk.queryNode", "arguments": ["2603231410"]}),
    )?;
    let expected: Value = serde_json::from_str(include_str!("fixtures/schema2/node.json"))?;
    assert_eq!(response["result"], expected);
    assert!(response.get("error").is_none());
    let hover = client.request("textDocument/hover", json!({"textDocument": {"uri": source_uri}, "position": position(source, source.find('@').unwrap() + 2, Encoding::Utf16)}))?;
    let text = hover["contents"]["value"].as_str().unwrap();
    assert!(text.contains("A note") && text.contains("2603231410"));
    assert!(text.find("summary").unwrap() < text.find("tags").unwrap());
    assert!(text.find("tags").unwrap() < text.find("topic").unwrap());
    assert!(text.contains("Summary") && text.contains("one") && text.contains("coding"));
    assert!(!text.contains("review"));
    let symbols = client.request("workspace/symbol", json!({"query": "two"}))?;
    assert_eq!(symbols.as_array().unwrap().len(), 1);
    assert!(symbols[0]["name"].as_str().unwrap().contains("A note"));
    assert!(symbols[0].get("containerName").is_none());
    client.shutdown()?;
    Ok(())
}

#[test]
fn exit_without_shutdown_has_status_one() -> Result<(), Box<dyn Error>> {
    for initialize in [false, true] {
        let temporary = tempdir()?;
        Archive::init(temporary.path())?;
        let mut client = LspClient::spawn(Path::new(env!("CARGO_BIN_EXE_zk")), temporary.path())?;
        if initialize {
            client.request("initialize", json!({"capabilities": {}}))?;
            client.notify("initialized", json!({}))?;
        }
        client.notify("exit", Value::Null)?;
        client.finish_exit(1)?;
    }
    Ok(())
}

#[test]
fn query_errors_follow_protocol_two_over_stdio() -> Result<(), Box<dyn Error>> {
    let temporary = tempdir()?;
    Archive::init(temporary.path())?;
    fs::write(
        temporary.path().join("zettel/2603231410.typ"),
        "= Source <2603231410>\n",
    )?;
    let mut client = LspClient::spawn(Path::new(env!("CARGO_BIN_EXE_zk")), temporary.path())?;
    let initialized = client.request("initialize", json!({"capabilities": {}}))?;
    assert_eq!(
        initialized["capabilities"]["experimental"]["zk"]["protocolVersion"],
        2
    );
    assert_eq!(
        initialized["capabilities"]["experimental"]["zk"]["dataSchemaVersion"],
        2
    );
    client.notify("initialized", json!({}))?;
    for args in [
        json!([]),
        json!([1]),
        json!([null]),
        json!(["2603231410", "extra"]),
        json!(["9999999999"]),
    ] {
        for command in ["zk.queryNode", "zk.links", "zk.backlinks"] {
            let error = client.request_error(
                "workspace/executeCommand",
                json!({"command": command, "arguments": args}),
            )?;
            assert_eq!(error["code"], -32602);
        }
    }
    for args in [json!([]), json!(["9999999999"])] {
        let error = client.request_error(
            "workspace/executeCommand",
            json!({"command": "zk.unknown", "arguments": args}),
        )?;
        assert_eq!(error["code"], -32601);
    }
    let error = client.request_error("zk.unknownMethod", json!({}))?;
    assert_eq!(error["code"], -32601);
    for command in ["zk.links", "zk.backlinks"] {
        let data = client.request(
            "workspace/executeCommand",
            json!({"command": command, "arguments": ["2603231410"]}),
        )?;
        assert_eq!(data, json!([]));
    }
    client.shutdown()?;
    Ok(())
}

#[test]
fn configurable_metadata_and_tinymist_boundary_over_stdio() -> Result<(), Box<dyn Error>> {
    let temporary = tempdir()?;
    let archive = temporary.path().join("archive");
    Archive::init(&archive)?;
    let archive = fs::canonicalize(archive)?;
    let manifest_path = archive.join("zk.toml");
    let source_path = archive.join("zettel/2603231410.typ");
    let target_path = archive.join("zettel/2603231411.typ");
    let session_path = archive.join("zettel/2603231412.typ");
    let source_uri = Url::from_file_path(&source_path).unwrap();
    let session_uri = Url::from_file_path(&session_path).unwrap();
    let manifest_uri = Url::from_file_path(&manifest_path).unwrap();
    let library_uri = Url::from_file_path(archive.join("lib/zettel.typ")).unwrap();
    fn config(abstract_name: &str) -> String {
        format!(
            "format = 2\n[metadata.summary]\nform = 'content-call'\nname = '{abstract_name}'\n\
                 [metadata.tags]\nform = 'string-array-call'\nname = 'tags'\n\
                 [metadata.topic]\nform = 'field-access'\nname = 'group'\n"
        )
    }
    fn dual(id: &str, title: &str) -> String {
        format!(
            "#import \"styles.typ\": extra\n= {title} <{id}>\n\
                 #summary[Old {title}]\nProse.\n#description[New {title}]\n\
                 #tags((\"custom\",))\n#group.coding\n"
        )
    }
    fs::write(&manifest_path, config("summary"))?;
    fs::write(&source_path, dual("2603231410", "Disk source"))?;
    fs::write(&target_path, dual("2603231411", "Disk target"))?;
    let mut client = LspClient::spawn(Path::new(env!("CARGO_BIN_EXE_zk")), &archive)?;
    client.request("initialize", json!({"capabilities": {"workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}}}}))?;
    client.notify("initialized", json!({}))?;
    let node = client.request(
        "workspace/executeCommand",
        json!({"command": "zk.queryNode", "arguments": ["2603231411"]}),
    )?;
    assert_eq!(
        node["metadata"]["summary"]["value"]["text"],
        "Old Disk target"
    );
    assert_eq!(node["metadata"]["tags"]["value"], json!(["custom"]));
    assert_eq!(node["metadata"]["topic"]["value"], "coding");

    // A syntax error unrelated to metadata belongs to Tinymist, not zk lsp.
    let overlay = format!("{}#let broken = (\n", dual("2603231410", "Overlay café"));
    client.notify("textDocument/didOpen", json!({"textDocument": {"uri": source_uri, "languageId": "typst", "version": 3, "text": overlay}}))?;
    let diagnostics = client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == source_uri.as_str() && params["version"] == 3
    })?;
    assert!(
        diagnostics["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let session = dual("2603231412", "Session source");
    client.notify("textDocument/didOpen", json!({"textDocument": {"uri": session_uri, "languageId": "typst", "version": 1, "text": session}}))?;
    client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == session_uri.as_str()
    })?;

    // The companion ignores ordinary library buffers and language-service requests.
    client.notify("textDocument/didOpen", json!({"textDocument": {"uri": library_uri, "languageId": "typst", "version": 1, "text": "#let category = (coding: [Coding])\n"}}))?;
    client.notify("textDocument/didChange", json!({"textDocument": {"uri": library_uri, "version": 2}, "contentChanges": [{"text": "#let category = ()\n"}]}))?;
    client.notify(
        "textDocument/didSave",
        json!({"textDocument": {"uri": library_uri}}),
    )?;
    for method in [
        "textDocument/completion",
        "textDocument/hover",
        "textDocument/definition",
    ] {
        let response = client.request(
            method,
            json!({"textDocument": {"uri": library_uri}, "position": {"line": 0, "character": 0}}),
        )?;
        assert!(response.is_null(), "{method} took over a library request");
    }
    client.notify(
        "textDocument/didClose",
        json!({"textDocument": {"uri": library_uri}}),
    )?;

    fs::write(&manifest_path, config("description"))?;
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": manifest_uri, "type": 2}]}),
    )?;
    client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == source_uri.as_str() && params["version"] == 3
    })?;
    for (id, expected) in [
        ("2603231410", "New Overlay café"),
        ("2603231411", "New Disk target"),
        ("2603231412", "New Session source"),
    ] {
        let node = client.request(
            "workspace/executeCommand",
            json!({"command": "zk.queryNode", "arguments": [id]}),
        )?;
        assert_eq!(node["metadata"]["summary"]["value"]["text"], expected);
    }
    let symbols = client.request("workspace/symbol", json!({"query": "New Overlay"}))?;
    assert_eq!(symbols.as_array().unwrap().len(), 1);

    // Invalid changes keep the last valid rules and live graph.
    fs::write(
        &manifest_path,
        "format = 2\n[metadata.summary]\nform = 'content-call'\nname = 'module.description'\n",
    )?;
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": manifest_uri, "type": 2}]}),
    )?;
    client.wait_notification("window/logMessage", |params| {
        params["type"] == 1
            && params["message"]
                .as_str()
                .is_some_and(|message| message.contains("invalid archive configuration"))
    })?;
    let node = client.request(
        "workspace/executeCommand",
        json!({"command": "zk.queryNode", "arguments": ["2603231410"]}),
    )?;
    assert_eq!(
        node["metadata"]["summary"]["value"]["text"],
        "New Overlay café"
    );

    let malformed = overlay.replace("#tags((\"custom\",))", "#tags((computed,))");
    client.notify("textDocument/didChange", json!({"textDocument": {"uri": source_uri, "version": 4}, "contentChanges": [{"text": malformed}]}))?;
    let diagnostics = client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == source_uri.as_str() && params["version"] == 4
    })?;
    let diagnostics = diagnostics["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], "metadata.invalid_shape");
    let offset = malformed.find("#tags").unwrap();
    assert_eq!(
        diagnostics[0]["data"],
        json!({
            "schema_version": 2,
            "path": "zettel/2603231410.typ",
            "field": "tags",
            "byte_range": {"start": offset, "end": offset + "#tags((computed,))".len()},
        })
    );
    assert_eq!(
        diagnostics[0]["range"]["start"],
        position(
            &malformed,
            malformed.find("#tags").unwrap(),
            Encoding::Utf16
        )
    );
    assert!(client.registrations.iter().any(|request| {
        request["params"]["registrations"][0]["registerOptions"]["watchers"]
            .as_array()
            .is_some_and(|watchers| {
                watchers
                    .iter()
                    .any(|watcher| watcher["globPattern"] == "**/zk.toml")
            })
    }));
    assert!(
        !client
            .notifications
            .iter()
            .any(|message| message["method"] == "window/logMessage"
                && message["params"]["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("didOpen")
                        || message.contains("didChange:")
                        || message.contains("didSave")
                        || message.contains("didClose")))
    );
    fs::write(&manifest_path, "format = 2\n")?;
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": manifest_uri, "type": 2}]}),
    )?;
    client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == source_uri.as_str()
            && params["version"] == 4
            && params["diagnostics"].as_array().is_some_and(Vec::is_empty)
    })?;
    for id in ["2603231410", "2603231411", "2603231412"] {
        let node = client.request(
            "workspace/executeCommand",
            json!({"command": "zk.queryNode", "arguments": [id]}),
        )?;
        assert_eq!(node["metadata"], json!({}));
    }
    assert_eq!(
        client.request("workspace/symbol", json!({"query": "custom"}))?,
        json!([])
    );
    client.shutdown()?;
    Ok(())
}

fn protocol_session(encoding: Encoding) -> Result<(), Box<dyn Error>> {
    let temporary = tempdir()?;
    let archive = temporary.path().join("archive");
    Archive::init(&archive)?;
    let archive = fs::canonicalize(archive)?;
    fs::write(
        archive.join("zettel/2603231410.typ"),
        zettel(
            "Disk source",
            "2603231410",
            "\"source\"",
            "Link @2603231411.",
        ),
    )?;
    write_target(&archive.join("zettel/2603231411.typ"), "Disk target")?;
    let binary = Path::new(env!("CARGO_BIN_EXE_zk"));
    let source_path = archive.join("zettel/2603231410.typ");
    let target_path = archive.join("zettel/2603231411.typ");
    let session_path = archive.join("zettel/2603231412.typ");
    let source_uri = Url::from_file_path(&source_path).map_err(|_| "source URI")?;
    let target_uri = Url::from_file_path(&target_path).map_err(|_| "target URI")?;
    let session_uri = Url::from_file_path(&session_path).map_err(|_| "session URI")?;
    let root_uri = Url::from_file_path(&archive).map_err(|_| "root URI")?;

    let library_path = archive.join("lib/zettel.typ");
    let library =
        fs::read_to_string(&library_path)?.replace("coding: [Coding]", "history: [History]");
    fs::write(&library_path, library)?;

    let mut client = LspClient::spawn(binary, &archive)?;
    let initialized = client.request(
        "initialize",
        json!({
            "processId": null,
            "rootUri": root_uri,
            "capabilities": {
                "general": {"positionEncodings": [encoding.name()]},
                "workspace": {
                    "didChangeWatchedFiles": {"dynamicRegistration": true}
                }
            }
        }),
    )?;
    assert_eq!(
        initialized["capabilities"]["positionEncoding"],
        encoding.name()
    );
    assert_eq!(initialized["capabilities"]["textDocumentSync"]["change"], 1);
    assert_eq!(
        initialized["capabilities"]["completionProvider"]["triggerCharacters"],
        json!(["@"])
    );
    assert_eq!(
        initialized["capabilities"]["experimental"]["zk"]["protocolVersion"],
        2
    );
    assert_eq!(
        initialized["capabilities"]["experimental"]["zk"]["features"],
        json!({
            "archiveQueries": true,
            "categoryCompletion": false,
            "referenceCompletion": true,
            "referenceTitleDecorations": true,
        })
    );
    client.notify("initialized", json!({}))?;

    let open_text = zettel(
        "Unsaved source",
        "2603231410",
        "computed",
        "Emoji 😀 links @2603231411 and misses @9999999999.\nComplete @2603\nSearch @DISK tar",
    )
    .replace("#category.thoughts", "#category.hi");
    client.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": source_uri,
                "languageId": "typst",
                "version": 1,
                "text": open_text,
            }
        }),
    )?;
    let diagnostics = client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == source_uri.as_str()
            && params["version"] == 1
            && params["diagnostics"].as_array().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item["code"] == "metadata.invalid_shape")
                    && items
                        .iter()
                        .any(|item| item["code"] == "reference.dangling")
            })
    })?;
    let missing_offset = open_text.find("@9999999999").unwrap();
    let expected_missing = position(&open_text, missing_offset, encoding);
    let dangling = diagnostics["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code"] == "reference.dangling")
        .unwrap();
    assert_eq!(dangling["range"]["start"], expected_missing);
    assert_eq!(
        dangling["data"],
        json!({
            "schema_version": 2,
            "path": "zettel/2603231410.typ",
            "field": null,
            "byte_range": {"start": missing_offset, "end": missing_offset + 11},
        })
    );
    let node = client.request(
        "workspace/executeCommand",
        json!({"command": "zk.queryNode", "arguments": ["2603231410"]}),
    )?;
    assert_eq!(node["title"]["text"], "Unsaved source");

    let completion_offset = open_text.rfind("@2603").unwrap() + "@2603".len();
    let completion = client.request(
        "textDocument/completion",
        json!({
            "textDocument": {"uri": source_uri},
            "position": position(&open_text, completion_offset, encoding),
        }),
    )?;
    assert_eq!(completion["isIncomplete"], true);
    assert!(
        completion["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item.get("documentation").is_none())
    );
    let target_completion = completion["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["label"] == "Disk target [2603231411]")
        .expect("target completion");
    assert_eq!(
        target_completion["textEdit"]["range"]["start"],
        position(&open_text, completion_offset - 4, encoding)
    );

    let title_completion_offset = open_text.rfind("@DISK tar").unwrap() + "@DISK tar".len();
    let title_completion = client.request(
        "textDocument/completion",
        json!({
            "textDocument": {"uri": source_uri},
            "position": position(&open_text, title_completion_offset, encoding),
            "context": {"triggerKind": 3},
        }),
    )?;
    assert_eq!(title_completion["isIncomplete"], true);
    let title_completion = &title_completion["items"][0];
    assert_eq!(title_completion["label"], "Disk target [2603231411]");
    assert_eq!(title_completion["textEdit"]["newText"], "2603231411");
    assert_eq!(
        title_completion["textEdit"]["range"]["start"],
        position(
            &open_text,
            title_completion_offset - "DISK tar".len(),
            encoding
        )
    );

    let category_completion_offset = open_text.find("#category.hi").unwrap() + "#category.hi".len();
    let category_completion = client.request(
        "textDocument/completion",
        json!({
            "textDocument": {"uri": source_uri},
            "position": position(&open_text, category_completion_offset, encoding),
        }),
    )?;
    assert!(category_completion.is_null());

    let reference_offset = open_text.find("@2603231411").unwrap() + 2;
    let reference_position = position(&open_text, reference_offset, encoding);
    let hover = client.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": source_uri}, "position": reference_position}),
    )?;
    let hover_text = hover["contents"]["value"].as_str().unwrap();
    assert!(hover_text.contains("Disk target"));
    assert!(hover_text.contains("keywords") && hover_text.contains("target"));

    let definition = client.request(
        "textDocument/definition",
        json!({"textDocument": {"uri": source_uri}, "position": reference_position}),
    )?;
    assert_eq!(definition["uri"], target_uri.as_str());

    let references = client.request(
        "textDocument/references",
        json!({
            "textDocument": {"uri": source_uri},
            "position": reference_position,
            "context": {"includeDeclaration": true},
        }),
    )?;
    assert_eq!(references.as_array().unwrap().len(), 2);

    let backlinks = client.request(
        "workspace/executeCommand",
        json!({"command": "zk.backlinks", "arguments": ["2603231411"]}),
    )?;
    assert_eq!(backlinks[0]["source"], "2603231410");

    let session_text = zettel(
        "Session target",
        "2603231412",
        "\"session\"",
        "Link @2603231411.",
    );
    client.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": session_uri,
                "languageId": "typst",
                "version": 1,
                "text": session_text,
            }
        }),
    )?;
    client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == session_uri.as_str()
            && params["version"] == 1
            && params["diagnostics"].as_array().is_some_and(Vec::is_empty)
    })?;
    let symbols = client.request("workspace/symbol", json!({"query": "session"}))?;
    assert_eq!(symbols.as_array().unwrap().len(), 1);
    assert_eq!(symbols[0]["location"]["uri"], session_uri.as_str());
    assert!(symbols[0].get("containerName").is_none());
    assert!(
        symbols[0]["name"]
            .as_str()
            .unwrap()
            .contains("Session target")
    );

    let changed_text = zettel(
        "Changed source",
        "2603231410",
        "\"fixed\"",
        "Emoji 😀 links @2603231412. Complete @2603",
    );
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": source_uri, "version": 2},
            "contentChanges": [{"text": changed_text}],
        }),
    )?;
    client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == source_uri.as_str()
            && params["version"] == 2
            && params["diagnostics"].as_array().is_some_and(Vec::is_empty)
    })?;
    let changed_node = client.request(
        "workspace/executeCommand",
        json!({"command": "zk.queryNode", "arguments": ["2603231410"]}),
    )?;
    assert_eq!(changed_node["title"]["text"], "Changed source");

    let changed_reference = changed_text.find("@2603231412").unwrap() + 2;
    let changed_position = position(&changed_text, changed_reference, encoding);
    let session_definition = client.request(
        "textDocument/definition",
        json!({"textDocument": {"uri": source_uri}, "position": changed_position}),
    )?;
    assert_eq!(session_definition["uri"], session_uri.as_str());

    let stale_text = zettel(
        "Stale source",
        "2603231410",
        "\"stale\"",
        "Link @2603231411.",
    );
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": source_uri, "version": 2},
            "contentChanges": [{"text": stale_text}],
        }),
    )?;
    client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == source_uri.as_str()
            && params["version"] == 2
            && params["diagnostics"].as_array().is_some_and(Vec::is_empty)
    })?;
    let after_stale = client.request(
        "workspace/executeCommand",
        json!({"command": "zk.queryNode", "arguments": ["2603231410"]}),
    )?;
    assert_eq!(after_stale["title"]["text"], "Changed source");

    client.notify(
        "textDocument/didSave",
        json!({"textDocument": {"uri": source_uri}}),
    )?;
    client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == source_uri.as_str()
            && params["version"] == 2
            && params["diagnostics"].as_array().is_some_and(Vec::is_empty)
    })?;
    client.notify(
        "textDocument/didClose",
        json!({"textDocument": {"uri": session_uri}}),
    )?;
    client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == source_uri.as_str()
            && params["version"] == 2
            && params["diagnostics"].as_array().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item["code"] == "reference.dangling")
            })
    })?;
    let missing_links = client.request(
        "workspace/executeCommand",
        json!({"command": "zk.links", "arguments": ["2603231410"]}),
    )?;
    assert_eq!(missing_links[0]["resolution"], "missing");
    let missing_hover = client.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": source_uri}, "position": changed_position}),
    )?;
    assert!(
        missing_hover["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("Missing Zettel")
    );

    write_target(&target_path, "Watched target")?;
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": target_uri, "type": 2}]}),
    )?;
    client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == source_uri.as_str()
            && params["version"] == 2
            && params["diagnostics"].as_array().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item["code"] == "reference.dangling")
            })
    })?;
    let watched_node = client.request(
        "workspace/executeCommand",
        json!({"command": "zk.queryNode", "arguments": ["2603231411"]}),
    )?;
    assert_eq!(watched_node["title"]["text"], "Watched target");

    client.notify(
        "textDocument/didClose",
        json!({"textDocument": {"uri": source_uri}}),
    )?;
    client.wait_notification("textDocument/publishDiagnostics", |params| {
        params["uri"] == source_uri.as_str()
            && params["version"].is_null()
            && params["diagnostics"].as_array().is_some_and(Vec::is_empty)
    })?;
    let disk_node = client.request(
        "workspace/executeCommand",
        json!({"command": "zk.queryNode", "arguments": ["2603231410"]}),
    )?;
    assert_eq!(disk_node["title"]["text"], "Disk source");
    assert!(client.registrations.iter().any(|request| {
        request["params"]["registrations"][0]["method"]
            == "workspace/didChangeWatchedFiles"
            && request["params"]["registrations"][0]["registerOptions"]["watchers"][0]
                ["globPattern"]
                == "**/zettel/*.typ"
    }));

    client.shutdown()?;
    Ok(())
}
