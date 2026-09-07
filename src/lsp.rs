use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde_json::Value;
use tower_lsp::jsonrpc::{Error as RpcError, Result as RpcResult};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use typst_syntax::{LinkedNode, Side, Source, SyntaxKind};

use crate::archive::{Archive, is_zettel_id};
use crate::model::{ByteRange, Link, Severity, ZettelNode};
use crate::provider::{Provider, ProviderError, UpdateOutcome};

const QUERY_NODE: &str = "zk.queryNode";
const QUERY_LINKS: &str = "zk.links";
const QUERY_BACKLINKS: &str = "zk.backlinks";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionEncoding {
    Utf8,
    Utf16,
}

impl PositionEncoding {
    fn lsp(self) -> PositionEncodingKind {
        match self {
            Self::Utf8 => PositionEncodingKind::UTF8,
            Self::Utf16 => PositionEncodingKind::UTF16,
        }
    }
}

#[derive(Debug)]
struct State {
    provider: Provider,
    root: PathBuf,
    encoding: PositionEncoding,
    open_versions: HashMap<Url, i32>,
    register_watcher: bool,
}

#[derive(Debug)]
struct Backend {
    client: Client,
    state: RwLock<State>,
}

pub async fn serve(archive: Archive) -> Result<(), ProviderError> {
    let root = archive.root().to_path_buf();
    let provider = Provider::load(&archive)?;
    let (service, socket) = LspService::new(move |client| Backend {
        client,
        state: RwLock::new(State {
            provider,
            root,
            encoding: PositionEncoding::Utf16,
            open_versions: HashMap::new(),
            register_watcher: false,
        }),
    });
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
    Ok(())
}

impl Backend {
    async fn publish_open_diagnostics(&self) {
        let publications = {
            let state = self.state.read().unwrap_or_else(|error| error.into_inner());
            state
                .open_versions
                .iter()
                .map(|(uri, version)| {
                    (
                        uri.clone(),
                        diagnostics_for_uri(&state, uri),
                        Some(*version),
                    )
                })
                .collect::<Vec<_>>()
        };
        for (uri, diagnostics, version) in publications {
            match diagnostics {
                Ok(diagnostics) => {
                    self.client
                        .publish_diagnostics(uri, diagnostics, version)
                        .await;
                }
                Err(error) => self.log_provider_error("publish diagnostics", error).await,
            }
        }
    }

    async fn log_provider_error(&self, operation: &str, error: impl std::fmt::Display) {
        self.client
            .log_message(MessageType::ERROR, format!("{operation}: {error}"))
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> RpcResult<InitializeResult> {
        let encoding = negotiate_encoding(&params);
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        state.encoding = encoding;
        state.register_watcher = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.did_change_watched_files)
            .and_then(|watched| watched.dynamic_registration)
            .unwrap_or(false);
        drop(state);
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: Some(encoding.lsp()),
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..Default::default()
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["@".to_owned()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        QUERY_NODE.to_owned(),
                        QUERY_LINKS.to_owned(),
                        QUERY_BACKLINKS.to_owned(),
                    ],
                    work_done_progress_options: Default::default(),
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "zk".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let register_watcher = self
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .register_watcher;
        if register_watcher {
            let options = DidChangeWatchedFilesRegistrationOptions {
                watchers: vec![FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/zettel/*.typ".to_owned()),
                    kind: Some(WatchKind::Create | WatchKind::Change | WatchKind::Delete),
                }],
            };
            let registration = Registration {
                id: "zk-zettel-files".to_owned(),
                method: "workspace/didChangeWatchedFiles".to_owned(),
                register_options: Some(
                    serde_json::to_value(options).expect("watch registration options serialize"),
                ),
            };
            if let Err(error) = self.client.register_capability(vec![registration]).await {
                self.log_provider_error("register file watcher", error)
                    .await;
            }
        }
        self.client
            .log_message(MessageType::INFO, "zk archive provider ready")
            .await;
    }

    async fn shutdown(&self) -> RpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let document = params.text_document;
        let path = match uri_path(&document.uri) {
            Ok(path) => path,
            Err(error) => {
                self.log_provider_error("didOpen", error).await;
                return;
            }
        };
        let result = {
            let mut state = self
                .state
                .write()
                .unwrap_or_else(|error| error.into_inner());
            match state
                .provider
                .open_buffer(path, document.version, document.text)
            {
                Ok(UpdateOutcome::Applied { .. }) => {
                    state.open_versions.insert(document.uri, document.version);
                    Ok(())
                }
                Ok(_) => Ok(()),
                Err(error) => Err(error),
            }
        };
        if let Err(error) = result {
            self.log_provider_error("didOpen", error).await;
            return;
        }
        self.publish_open_diagnostics().await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let document = params.text_document;
        let Some(change) = params.content_changes.last() else {
            return;
        };
        if change.range.is_some() {
            self.log_provider_error("didChange", "incremental changes are not supported")
                .await;
            return;
        }
        let path = match uri_path(&document.uri) {
            Ok(path) => path,
            Err(error) => {
                self.log_provider_error("didChange", error).await;
                return;
            }
        };
        let result = {
            let mut state = self
                .state
                .write()
                .unwrap_or_else(|error| error.into_inner());
            match state
                .provider
                .change_buffer(path, document.version, &change.text)
            {
                Ok(UpdateOutcome::Applied { .. }) => {
                    state.open_versions.insert(document.uri, document.version);
                    Ok(())
                }
                Ok(UpdateOutcome::StaleVersion { .. }) => Ok(()),
                Ok(_) => Ok(()),
                Err(error) => Err(error),
            }
        };
        if let Err(error) = result {
            self.log_provider_error("didChange", error).await;
            return;
        }
        self.publish_open_diagnostics().await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let path = match uri_path(&params.text_document.uri) {
            Ok(path) => path,
            Err(error) => {
                self.log_provider_error("didSave", error).await;
                return;
            }
        };
        let result = self
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .provider
            .save_buffer(path);
        if let Err(error) = result {
            self.log_provider_error("didSave", error).await;
            return;
        }
        self.publish_open_diagnostics().await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let path = match uri_path(&uri) {
            Ok(path) => path,
            Err(error) => {
                self.log_provider_error("didClose", error).await;
                return;
            }
        };
        let result = {
            let mut state = self
                .state
                .write()
                .unwrap_or_else(|error| error.into_inner());
            let version = state.open_versions.remove(&uri);
            let result = state.provider.close_buffer(path);
            if result.is_err()
                && let Some(version) = version
            {
                state.open_versions.insert(uri.clone(), version);
            }
            result
        };
        if let Err(error) = result {
            self.log_provider_error("didClose", error).await;
            return;
        }
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
        self.publish_open_diagnostics().await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        for change in params.changes {
            let path = match uri_path(&change.uri) {
                Ok(path) => path,
                Err(_) => continue,
            };
            let result = self
                .state
                .write()
                .unwrap_or_else(|error| error.into_inner())
                .provider
                .refresh_disk(path);
            if let Err(error) = result {
                if matches!(error, ProviderError::NoncanonicalPath(_)) {
                    continue;
                }
                self.log_provider_error("didChangeWatchedFiles", error)
                    .await;
            }
        }
        self.publish_open_diagnostics().await;
    }

    async fn completion(&self, params: CompletionParams) -> RpcResult<Option<CompletionResponse>> {
        let state = self.state.read().unwrap_or_else(|error| error.into_inner());
        let uri = &params.text_document_position.text_document.uri;
        let path = uri_path(uri).map_err(RpcError::invalid_params)?;
        let text = document_text(&state, &path).map_err(internal_error)?;
        let offset = position_to_offset(
            &text,
            params.text_document_position.position,
            state.encoding,
        )
        .ok_or_else(|| RpcError::invalid_params("position is outside the document"))?;
        let source = state
            .provider
            .overlay_source(&path)
            .cloned()
            .unwrap_or_else(|| Source::detached(text.to_string()));
        let Some((replace_start, prefix)) = completion_context(&source, offset) else {
            return Ok(None);
        };
        let replace_range = byte_range_to_lsp(
            &text,
            ByteRange {
                start: replace_start as u32,
                end: offset as u32,
            },
            state.encoding,
        )
        .ok_or_else(RpcError::internal_error)?;

        let items = state
            .provider
            .nodes()
            .iter()
            .filter(|node| node.id.starts_with(prefix))
            .map(|node| {
                let title = node
                    .title
                    .as_ref()
                    .map(|title| title.text.as_str())
                    .unwrap_or("Untitled");
                CompletionItem {
                    label: node.id.clone(),
                    kind: Some(CompletionItemKind::REFERENCE),
                    detail: Some(title.to_owned()),
                    documentation: node
                        .abstract_value
                        .as_ref()
                        .map(|abstract_value| Documentation::String(abstract_value.text.clone())),
                    sort_text: Some(node.id.clone()),
                    filter_text: Some(node.id.clone()),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: replace_range,
                        new_text: node.id.clone(),
                    })),
                    ..Default::default()
                }
            })
            .collect();
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> RpcResult<Option<Hover>> {
        let state = self.state.read().unwrap_or_else(|error| error.into_inner());
        let position = params.text_document_position_params;
        let Some((link, span, text)) = reference_at(&state, &position)? else {
            return Ok(None);
        };
        let Some(node) = state.provider.node(&link.target) else {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("Missing Zettel `{}`", link.target),
                }),
                range: byte_range_to_lsp(&text, span, state.encoding),
            }));
        };
        let title = node
            .title
            .as_ref()
            .map(|title| title.text.as_str())
            .unwrap_or("Untitled");
        let mut value = format!("### {title}\n\n`{}`", node.id);
        if let Some(abstract_value) = &node.abstract_value
            && !abstract_value.text.is_empty()
        {
            value.push_str("\n\n");
            value.push_str(&abstract_value.text);
        }
        if let Some(keywords) = &node.keywords
            && !keywords.is_empty()
        {
            value.push_str("\n\nKeywords: ");
            value.push_str(
                &keywords
                    .iter()
                    .map(|keyword| format!("`{keyword}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if let Some(category) = &node.category {
            value.push_str(&format!("\n\nCategory: `{category}`"));
        }
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: byte_range_to_lsp(&text, span, state.encoding),
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> RpcResult<Option<GotoDefinitionResponse>> {
        let state = self.state.read().unwrap_or_else(|error| error.into_inner());
        let position = params.text_document_position_params;
        let Some((link, _, _)) = reference_at(&state, &position)? else {
            return Ok(None);
        };
        let Some(node) = state.provider.node(&link.target) else {
            return Ok(None);
        };
        let range = node
            .title
            .as_ref()
            .map(|title| title.range)
            .unwrap_or(ByteRange { start: 0, end: 0 });
        let location = location(&state, &node.path, range)?;
        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    }

    async fn references(&self, params: ReferenceParams) -> RpcResult<Option<Vec<Location>>> {
        let state = self.state.read().unwrap_or_else(|error| error.into_inner());
        let position = params.text_document_position;
        let Some((link, _, _)) = reference_at(&state, &position)? else {
            return Ok(None);
        };
        let mut locations = Vec::new();
        if params.context.include_declaration
            && let Some(node) = state.provider.node(&link.target)
        {
            let range = node
                .title
                .as_ref()
                .map(|title| title.range)
                .unwrap_or(ByteRange { start: 0, end: 0 });
            locations.push(location(&state, &node.path, range)?);
        }
        for incoming in state.provider.links_to(&link.target) {
            let source = state
                .provider
                .node(&incoming.source)
                .expect("incoming link source exists");
            for span in incoming.spans {
                locations.push(location(&state, &source.path, span)?);
            }
        }
        Ok(Some(locations))
    }

    #[allow(deprecated)]
    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> RpcResult<Option<Vec<SymbolInformation>>> {
        let state = self.state.read().unwrap_or_else(|error| error.into_inner());
        let query = params.query.to_lowercase();
        let mut symbols = Vec::new();
        for node in state
            .provider
            .nodes()
            .iter()
            .filter(|node| node_matches(node, &query))
        {
            let title = node
                .title
                .as_ref()
                .map(|title| title.text.as_str())
                .unwrap_or("Untitled");
            let range = node
                .title
                .as_ref()
                .map(|title| title.range)
                .unwrap_or(ByteRange { start: 0, end: 0 });
            symbols.push(SymbolInformation {
                name: format!("{title} [{}]", node.id),
                kind: SymbolKind::OBJECT,
                tags: None,
                deprecated: None,
                location: location(&state, &node.path, range)?,
                container_name: node.category.clone(),
            });
        }
        Ok(Some(symbols))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> RpcResult<Option<Value>> {
        let id = params
            .arguments
            .first()
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError::invalid_params("first argument must be a Zettel ID"))?;
        let state = self.state.read().unwrap_or_else(|error| error.into_inner());
        let node = state
            .provider
            .node(id)
            .ok_or_else(|| RpcError::invalid_params(format!("Zettel `{id}` does not exist")))?;
        let value = match params.command.as_str() {
            QUERY_NODE => serde_json::to_value(node),
            QUERY_LINKS => serde_json::to_value(state.provider.links_from(id)),
            QUERY_BACKLINKS => serde_json::to_value(state.provider.links_to(id)),
            _ => return Err(RpcError::method_not_found()),
        }
        .map_err(internal_error)?;
        Ok(Some(value))
    }
}

fn node_matches(node: &ZettelNode, query: &str) -> bool {
    if query.is_empty() || node.id.contains(query) {
        return true;
    }
    node.title
        .as_ref()
        .is_some_and(|title| title.text.to_lowercase().contains(query))
        || node
            .abstract_value
            .as_ref()
            .is_some_and(|abstract_value| abstract_value.text.to_lowercase().contains(query))
        || node.keywords.as_ref().is_some_and(|keywords| {
            keywords
                .iter()
                .any(|keyword| keyword.to_lowercase().contains(query))
        })
        || node
            .category
            .as_ref()
            .is_some_and(|category| category.to_lowercase().contains(query))
}

fn negotiate_encoding(params: &InitializeParams) -> PositionEncoding {
    let supports_utf8 = params
        .capabilities
        .general
        .as_ref()
        .and_then(|general| general.position_encodings.as_ref())
        .is_some_and(|encodings| encodings.contains(&PositionEncodingKind::UTF8));
    if supports_utf8 {
        PositionEncoding::Utf8
    } else {
        PositionEncoding::Utf16
    }
}

fn reference_at(
    state: &State,
    position: &TextDocumentPositionParams,
) -> RpcResult<Option<(Link, ByteRange, String)>> {
    let path = uri_path(&position.text_document.uri).map_err(RpcError::invalid_params)?;
    let id = zettel_id(&path)
        .ok_or_else(|| RpcError::invalid_params("document is not a canonical Zettel"))?;
    let text = document_text(state, &path).map_err(internal_error)?;
    let offset = position_to_offset(&text, position.position, state.encoding)
        .ok_or_else(|| RpcError::invalid_params("position is outside the document"))?;
    for link in state.provider.links_from(id) {
        if let Some(span) = link
            .spans
            .iter()
            .copied()
            .find(|span| span.start as usize <= offset && offset < span.end as usize)
        {
            return Ok(Some((link, span, text.into_owned())));
        }
    }
    Ok(None)
}

fn diagnostics_for_uri(
    state: &State,
    uri: &Url,
) -> Result<Vec<tower_lsp::lsp_types::Diagnostic>, String> {
    let path = uri_path(uri)?;
    let relative = archive_relative(&state.root, &path)?;
    let text = document_text(state, &path)?;
    Ok(state
        .provider
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.path == relative)
        .map(|diagnostic| tower_lsp::lsp_types::Diagnostic {
            range: diagnostic
                .range
                .and_then(|range| byte_range_to_lsp(&text, range, state.encoding))
                .unwrap_or_default(),
            severity: Some(match diagnostic.severity {
                Severity::Error => DiagnosticSeverity::ERROR,
                Severity::Warning => DiagnosticSeverity::WARNING,
            }),
            code: Some(NumberOrString::String(diagnostic.code.clone())),
            source: Some("zk".to_owned()),
            message: diagnostic.message.clone(),
            ..Default::default()
        })
        .collect())
}

fn location(state: &State, relative: &str, range: ByteRange) -> RpcResult<Location> {
    let path = state.root.join(relative);
    let uri = Url::from_file_path(&path).map_err(|_| {
        RpcError::invalid_params(format!("cannot create URI for {}", path.display()))
    })?;
    let text = document_text(state, &path).map_err(internal_error)?;
    let range =
        byte_range_to_lsp(&text, range, state.encoding).ok_or_else(RpcError::internal_error)?;
    Ok(Location { uri, range })
}

fn document_text<'a>(state: &'a State, path: &Path) -> Result<Cow<'a, str>, String> {
    if let Some(source) = state.provider.overlay_source(path) {
        return Ok(Cow::Borrowed(source.text()));
    }
    fs::read_to_string(path)
        .map(Cow::Owned)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))
}

fn completion_context(source: &Source, offset: usize) -> Option<(usize, &str)> {
    let linked = LinkedNode::new(source.root());
    if let Some(mut leaf) = linked.leaf_at(offset, Side::Before) {
        loop {
            if matches!(
                leaf.kind(),
                SyntaxKind::Raw
                    | SyntaxKind::Str
                    | SyntaxKind::LineComment
                    | SyntaxKind::BlockComment
                    | SyntaxKind::Escape
            ) {
                return None;
            }
            let Some(parent) = leaf.parent().cloned() else {
                break;
            };
            leaf = parent;
        }
    }
    let text = source.text();
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let before = &text[line_start..offset];
    let marker = before.rfind('@')?;
    let prefix = &before[marker + 1..];
    if prefix.len() > 10 || !prefix.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((line_start + marker + 1, prefix))
}

fn byte_range_to_lsp(text: &str, range: ByteRange, encoding: PositionEncoding) -> Option<Range> {
    Some(Range {
        start: offset_to_position(text, range.start as usize, encoding)?,
        end: offset_to_position(text, range.end as usize, encoding)?,
    })
}

fn offset_to_position(text: &str, offset: usize, encoding: PositionEncoding) -> Option<Position> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }
    let head = &text[..offset];
    let line = head.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = head.rfind('\n').map_or(0, |index| index + 1);
    let line_head = &text[line_start..offset];
    let character = match encoding {
        PositionEncoding::Utf8 => line_head.len(),
        PositionEncoding::Utf16 => line_head.encode_utf16().count(),
    };
    Some(Position::new(
        line.try_into().ok()?,
        character.try_into().ok()?,
    ))
}

fn position_to_offset(text: &str, position: Position, encoding: PositionEncoding) -> Option<usize> {
    let mut line_start = 0;
    for _ in 0..position.line {
        line_start += text[line_start..].find('\n')? + 1;
    }
    let line_end = text[line_start..]
        .find('\n')
        .map_or(text.len(), |index| line_start + index);
    let line = &text[line_start..line_end];
    let relative = match encoding {
        PositionEncoding::Utf8 => {
            let value = usize::try_from(position.character).ok()?;
            (value <= line.len() && line.is_char_boundary(value)).then_some(value)?
        }
        PositionEncoding::Utf16 => {
            let target = usize::try_from(position.character).ok()?;
            let mut units = 0;
            let mut bytes = 0;
            for character in line.chars() {
                if units == target {
                    break;
                }
                units += character.len_utf16();
                bytes += character.len_utf8();
                if units > target {
                    return None;
                }
            }
            (units == target).then_some(bytes)?
        }
    };
    Some(line_start + relative)
}

fn zettel_id(path: &Path) -> Option<&str> {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".typ"))
        .filter(|id| is_zettel_id(id))
}

fn archive_relative(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map(|relative| {
            relative
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/")
        })
        .map_err(|_| format!("{} is outside archive {}", path.display(), root.display()))
}

fn uri_path(uri: &Url) -> Result<PathBuf, String> {
    let path = uri
        .to_file_path()
        .map_err(|_| format!("URI is not a file path: {uri}"))?;
    if path.exists() {
        return fs::canonicalize(&path)
            .map_err(|error| format!("cannot resolve {}: {error}", path.display()));
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("file path has no parent: {}", path.display()))?;
    let parent = fs::canonicalize(parent)
        .map_err(|error| format!("cannot resolve {}: {error}", parent.display()))?;
    let filename = path
        .file_name()
        .ok_or_else(|| format!("file path has no name: {}", path.display()))?;
    Ok(parent.join(filename))
}

fn internal_error(error: impl std::fmt::Display) -> RpcError {
    let mut rpc = RpcError::internal_error();
    rpc.message = error.to_string().into();
    rpc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_utf8_and_utf16_positions() {
        let text = "first\ncafé 😀 end\n";
        let byte = text.find('😀').unwrap();

        let utf8 = offset_to_position(text, byte, PositionEncoding::Utf8).unwrap();
        let utf16 = offset_to_position(text, byte, PositionEncoding::Utf16).unwrap();

        assert_eq!(utf8, Position::new(1, 6));
        assert_eq!(utf16, Position::new(1, 5));
        assert_eq!(
            position_to_offset(text, utf8, PositionEncoding::Utf8),
            Some(byte)
        );
        assert_eq!(
            position_to_offset(text, utf16, PositionEncoding::Utf16),
            Some(byte)
        );
    }

    #[test]
    fn rejects_positions_inside_utf8_and_utf16_characters() {
        let text = "😀";
        assert!(position_to_offset(text, Position::new(0, 1), PositionEncoding::Utf8).is_none());
        assert!(position_to_offset(text, Position::new(0, 1), PositionEncoding::Utf16).is_none());
    }

    #[test]
    fn finds_completion_prefix_outside_raw_text_and_comments() {
        let source = Source::detached("Text @2603");
        assert_eq!(
            completion_context(&source, source.text().len()),
            Some((6, "2603"))
        );

        let raw = Source::detached("`@2603`");
        assert_eq!(completion_context(&raw, 6), None);

        let comment = Source::detached("// @2603");
        assert_eq!(completion_context(&comment, comment.text().len()), None);
    }
}
