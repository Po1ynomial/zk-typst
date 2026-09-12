use std::error::Error;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use serde_json::{Value, json};
use tower_lsp::lsp_types::Url;

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

struct Probe {
    child: Child,
    input: BufWriter<ChildStdin>,
    messages: Receiver<Result<Value, String>>,
    notifications: Vec<Value>,
    registrations: Vec<Value>,
    next_id: u64,
}

impl Probe {
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
            input,
            messages,
            notifications: Vec::new(),
            registrations: Vec::new(),
            next_id: 1,
        })
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, Box<dyn Error>> {
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
                if let Some(error) = message.get("error") {
                    return Err(format!("{method} failed: {error}").into());
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
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
        self.request("shutdown", Value::Null)?;
        self.notify("exit", Value::Null)?;
        drop(self.input);
        let status = self.child.wait()?;
        if !status.success() {
            return Err(format!("language server exited with {status}").into());
        }
        Ok(())
    }

    fn send(&mut self, message: Value) -> Result<(), Box<dyn Error>> {
        let body = serde_json::to_vec(&message)?;
        write!(self.input, "Content-Length: {}\r\n\r\n", body.len())?;
        self.input.write_all(&body)?;
        self.input.flush()?;
        Ok(())
    }

    fn receive(&self) -> Result<Value, Box<dyn Error>> {
        match self.messages.recv_timeout(Duration::from_secs(5))? {
            Ok(message) => Ok(message),
            Err(error) => Err(error.into()),
        }
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

#abstract[Live LSP inspection.]

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

fn main() -> Result<(), Box<dyn Error>> {
    let binary = PathBuf::from(std::env::args().nth(1).expect("zk binary argument"));
    let archive = fs::canonicalize(PathBuf::from(
        std::env::args().nth(2).expect("archive argument"),
    ))?;
    let encoding = match std::env::args().nth(3).as_deref() {
        Some("utf-8") => Encoding::Utf8,
        Some("utf-16") => Encoding::Utf16,
        other => return Err(format!("unsupported probe encoding: {other:?}").into()),
    };
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

    let mut client = Probe::spawn(&binary, &archive)?;
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
        initialized["capabilities"]["experimental"]["zk"]["protocolVersion"],
        1
    );
    assert_eq!(
        initialized["capabilities"]["experimental"]["zk"]["features"]["archiveQueries"],
        true
    );
    assert_eq!(
        initialized["capabilities"]["experimental"]["zk"]["features"]["referenceTitleDecorations"],
        true
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
                items.iter().any(|item| item["code"] == "metadata.keywords")
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
    assert_eq!(category_completion["items"].as_array().unwrap().len(), 1);
    assert_eq!(category_completion["items"][0]["label"], "history");
    assert_eq!(
        category_completion["items"][0]["textEdit"]["newText"],
        "history"
    );

    let reference_offset = open_text.find("@2603231411").unwrap() + 2;
    let reference_position = position(&open_text, reference_offset, encoding);
    let hover = client.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": source_uri}, "position": reference_position}),
    )?;
    let hover_text = hover["contents"]["value"].as_str().unwrap();
    assert!(hover_text.contains("Disk target"));
    assert!(hover_text.contains("Keywords: `target`"));

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
    serde_json::to_writer_pretty(
        std::io::stdout().lock(),
        &json!({
            "encoding": encoding.name(),
            "completion": true,
            "referenceCompletion": true,
            "categoryCompletion": true,
            "hover": true,
            "definition": true,
            "references": true,
            "backlinks": true,
            "archiveSearch": true,
            "diagnostics": true,
            "unsavedState": true,
            "staleVersionRejected": true,
            "watchedFileRefresh": true,
            "watcherRegistration": true,
            "shutdown": true,
        }),
    )?;
    println!();
    Ok(())
}
