use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};

use rayon::prelude::*;
use thiserror::Error;
use typst_syntax::Source;

use crate::archive::{Archive, ArchiveError, is_zettel_id};
use crate::config::MetadataContract;
use crate::extract::{ExtractedNode, extract, extract_source};
use crate::model::{
    ByteRange, Diagnostic, GraphSnapshot, Link, LinkResolution, MAX_JSON_INTEGER, Severity,
    ZettelNode, compare_diagnostics,
};

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),

    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Zettel source exceeds the 4 GiB byte-range limit: {0}")]
    SourceTooLarge(PathBuf),

    #[error("path is not a canonical Zettel path: {0}")]
    NoncanonicalPath(PathBuf),

    #[error("buffer is not open: {0}")]
    BufferNotOpen(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OutgoingLink {
    target: u32,
    span_start: u32,
    span_len: u32,
}

#[derive(Debug, Clone)]
struct Overlay {
    version: i32,
    generation: u64,
    source: Source,
}

#[derive(Debug)]
enum PreparedChange {
    Replace(Box<ExtractedNode>),
    Remove,
}

#[derive(Debug)]
pub struct PreparedUpdate {
    id: String,
    generation: u64,
    change: PreparedChange,
}

impl PreparedUpdate {
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOutcome {
    Applied { generation: u64, revision: u64 },
    StaleVersion { current: i32 },
    StaleGeneration { current: u64 },
    OverlayRetained { generation: u64, revision: u64 },
    IgnoredOpenOverlay,
}

#[derive(Debug, Clone)]
pub struct Provider {
    archive_root: PathBuf,
    metadata: MetadataContract,
    revision: u64,
    nodes: Vec<ZettelNode>,
    diagnostics: Vec<Diagnostic>,
    archive_diagnostics: Vec<Diagnostic>,
    ids: Vec<String>,
    id_indices: HashMap<String, u32>,
    source_diagnostics: Vec<Vec<Diagnostic>>,
    graph_diagnostics: Vec<Vec<Diagnostic>>,
    present: Vec<bool>,
    outgoing: Vec<Vec<OutgoingLink>>,
    spans: Vec<Vec<ByteRange>>,
    incoming: Vec<Vec<u32>>,
    generations: Vec<u64>,
    overlays: HashMap<u32, Overlay>,
    next_generation: u64,
}

impl Provider {
    pub fn load(archive: &Archive) -> Result<Self, ProviderError> {
        let archive = Archive::open(archive.root())?;
        let discovered = discover_paths(archive.root())?;
        let extracted: Result<Vec<_>, _> = discovered
            .canonical
            .par_iter()
            .map(|(id, path)| load_zettel(archive.root(), id, path, archive.metadata_contract()))
            .collect();

        let mut provider = Self {
            archive_root: archive.root().to_path_buf(),
            metadata: archive.metadata_contract().clone(),
            revision: 1,
            nodes: Vec::new(),
            diagnostics: Vec::new(),
            archive_diagnostics: discovered.diagnostics,
            ids: Vec::new(),
            id_indices: HashMap::new(),
            source_diagnostics: Vec::new(),
            graph_diagnostics: Vec::new(),
            present: Vec::new(),
            outgoing: Vec::new(),
            spans: Vec::new(),
            incoming: Vec::new(),
            generations: Vec::new(),
            overlays: HashMap::new(),
            next_generation: 2,
        };
        for extracted in extracted? {
            let source = provider.intern(&extracted.node.id);
            provider.generations[source as usize] = 1;
            provider.install(extracted);
        }
        provider.rebuild_all_graph_diagnostics();
        provider.rebuild_diagnostic_list();
        Ok(provider)
    }

    /// Reload saved extraction rules atomically, preserving all open sources and versions.
    /// A failed reload leaves the previous graph and rules intact.
    pub fn reload_manifest(&mut self) -> Result<bool, ProviderError> {
        let archive = Archive::open(&self.archive_root)?;
        let metadata = archive.metadata_contract();
        if metadata == &self.metadata {
            return Ok(false);
        }
        let discovered = discover_paths(&self.archive_root)?;
        let extracted: Result<Vec<_>, _> = discovered
            .canonical
            .par_iter()
            .filter(|(id, _)| {
                !self
                    .id_indices
                    .get(id)
                    .is_some_and(|index| self.overlays.contains_key(index))
            })
            .map(|(id, path)| load_zettel(&self.archive_root, id, path, metadata))
            .collect();
        let mut extracted = extracted?;
        for (index, overlay) in &self.overlays {
            let id = &self.ids[*index as usize];
            extracted.push(extract_source(
                id,
                &format!("zettel/{id}.typ"),
                &overlay.source,
                overlay.generation,
                metadata,
            ));
        }
        // All fallible reads finish before changing the live graph.
        let retained: BTreeSet<_> = extracted
            .iter()
            .map(|value| value.node.id.clone())
            .collect();
        for value in &extracted {
            self.intern(&value.node.id);
        }
        self.metadata = metadata.clone();
        self.archive_diagnostics = discovered.diagnostics;
        // Invalidate every prepared result, including results for absent nodes.
        for index in 0..self.ids.len() as u32 {
            let generation = self.schedule(index);
            if let Some(overlay) = self.overlays.get_mut(&index) {
                overlay.generation = generation;
            }
            if self.present[index as usize] && !retained.contains(&self.ids[index as usize]) {
                let id = self.ids[index as usize].clone();
                self.remove_node(index, &id);
            }
        }
        for mut value in extracted {
            value.node.generation = self.generations[self.id_indices[&value.node.id] as usize];
            self.install(value);
        }
        self.revision = self
            .revision
            .checked_add(1)
            .filter(|revision| *revision <= MAX_JSON_INTEGER)
            .expect("JSON-safe graph revision exhausted");
        self.rebuild_all_graph_diagnostics();
        self.rebuild_diagnostic_list();
        Ok(true)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn nodes(&self) -> &[ZettelNode] {
        &self.nodes
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn node(&self, id: &str) -> Option<&ZettelNode> {
        self.nodes
            .binary_search_by(|node| node.id.as_str().cmp(id))
            .ok()
            .map(|index| &self.nodes[index])
    }

    pub fn search_metadata(&self, query: &str) -> Vec<&ZettelNode> {
        let query = query.to_lowercase();
        self.nodes
            .iter()
            .filter(|node| node_matches(node, &query))
            .collect()
    }

    pub fn links_from(&self, id: &str) -> Vec<Link> {
        let Some(&source) = self.id_indices.get(id) else {
            return Vec::new();
        };
        let mut links: Vec<_> = self.outgoing[source as usize]
            .iter()
            .map(|link| self.public_link(source, link))
            .collect();
        links.sort_by(|left, right| left.target.cmp(&right.target));
        links
    }

    pub fn links_to(&self, id: &str) -> Vec<Link> {
        let Some(&target) = self.id_indices.get(id) else {
            return Vec::new();
        };
        let mut links: Vec<_> = self.incoming[target as usize]
            .iter()
            .filter_map(|source| {
                let link = self.outgoing[*source as usize]
                    .binary_search_by_key(&target, |link| link.target)
                    .ok()
                    .map(|index| &self.outgoing[*source as usize][index])?;
                Some(self.public_link(*source, link))
            })
            .collect();
        links.sort_by(|left, right| left.source.cmp(&right.source));
        links
    }

    pub fn snapshot(&self) -> GraphSnapshot {
        let mut links = Vec::new();
        for node in &self.nodes {
            links.extend(self.links_from(&node.id));
        }
        links.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.target.cmp(&right.target))
        });
        GraphSnapshot {
            revision: self.revision,
            nodes: self.nodes.clone(),
            links,
            diagnostics: self.diagnostics.clone(),
        }
    }

    pub fn open_buffer(
        &mut self,
        path: impl AsRef<Path>,
        version: i32,
        text: String,
    ) -> Result<UpdateOutcome, ProviderError> {
        let document = self.document(path.as_ref())?;
        let source = self.intern(&document.id);
        if let Some(overlay) = self.overlays.get(&source)
            && version <= overlay.version
        {
            return Ok(UpdateOutcome::StaleVersion {
                current: overlay.version,
            });
        }
        ensure_source_size(&document.absolute, &text)?;

        let generation = self.schedule(source);
        let extracted = if let Some(overlay) = self.overlays.get_mut(&source) {
            overlay.source.replace(&text);
            overlay.version = version;
            overlay.generation = generation;
            extract_source(
                &document.id,
                &document.relative,
                &overlay.source,
                generation,
                &self.metadata,
            )
        } else {
            let source_text = checked_source(&document.absolute, text)?;
            let source_file = Source::detached(source_text);
            let extracted = extract_source(
                &document.id,
                &document.relative,
                &source_file,
                generation,
                &self.metadata,
            );
            self.overlays.insert(
                source,
                Overlay {
                    version,
                    generation,
                    source: source_file,
                },
            );
            extracted
        };
        let affected = self.install(extracted);
        Ok(self.finish_update(generation, affected))
    }

    pub fn change_buffer(
        &mut self,
        path: impl AsRef<Path>,
        version: i32,
        text: &str,
    ) -> Result<UpdateOutcome, ProviderError> {
        let document = self.document(path.as_ref())?;
        let Some(&source) = self.id_indices.get(&document.id) else {
            return Err(ProviderError::BufferNotOpen(document.absolute));
        };
        let Some(overlay) = self.overlays.get(&source) else {
            return Err(ProviderError::BufferNotOpen(document.absolute));
        };
        if version <= overlay.version {
            return Ok(UpdateOutcome::StaleVersion {
                current: overlay.version,
            });
        }
        ensure_source_size(&document.absolute, text)?;

        let generation = self.schedule(source);
        let overlay = self
            .overlays
            .get_mut(&source)
            .expect("open overlay checked");
        overlay.source.replace(text);
        overlay.version = version;
        overlay.generation = generation;
        let extracted = extract_source(
            &document.id,
            &document.relative,
            &overlay.source,
            generation,
            &self.metadata,
        );
        let affected = self.install(extracted);
        Ok(self.finish_update(generation, affected))
    }

    pub fn save_buffer(&self, path: impl AsRef<Path>) -> Result<UpdateOutcome, ProviderError> {
        let document = self.document(path.as_ref())?;
        let Some(&source) = self.id_indices.get(&document.id) else {
            return Err(ProviderError::BufferNotOpen(document.absolute));
        };
        let Some(overlay) = self.overlays.get(&source) else {
            return Err(ProviderError::BufferNotOpen(document.absolute));
        };
        Ok(UpdateOutcome::OverlayRetained {
            generation: overlay.generation,
            revision: self.revision,
        })
    }

    pub fn close_buffer(&mut self, path: impl AsRef<Path>) -> Result<UpdateOutcome, ProviderError> {
        let document = self.document(path.as_ref())?;
        let Some(&source) = self.id_indices.get(&document.id) else {
            return Err(ProviderError::BufferNotOpen(document.absolute));
        };
        let Some(overlay) = self.overlays.remove(&source) else {
            return Err(ProviderError::BufferNotOpen(document.absolute));
        };
        match self.prepare_disk_update(&document.absolute) {
            Ok(Some(update)) => Ok(self.apply_prepared(update)),
            Ok(None) => unreachable!("closed buffers accept disk updates"),
            Err(error) => {
                self.generations[source as usize] = overlay.generation;
                self.overlays.insert(source, overlay);
                Err(error)
            }
        }
    }

    pub fn refresh_disk(&mut self, path: impl AsRef<Path>) -> Result<UpdateOutcome, ProviderError> {
        match self.prepare_disk_update(path)? {
            Some(update) => Ok(self.apply_prepared(update)),
            None => Ok(UpdateOutcome::IgnoredOpenOverlay),
        }
    }

    pub fn prepare_disk_update(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<Option<PreparedUpdate>, ProviderError> {
        let document = self.document(path.as_ref())?;
        let source = self.intern(&document.id);
        if self.overlays.contains_key(&source) {
            return Ok(None);
        }
        let generation = self.schedule(source);
        let change = match fs::read_to_string(&document.absolute) {
            Ok(text) => {
                let text = checked_source(&document.absolute, text)?;
                let source = Source::detached(text);
                PreparedChange::Replace(Box::new(extract_source(
                    &document.id,
                    &document.relative,
                    &source,
                    generation,
                    &self.metadata,
                )))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => PreparedChange::Remove,
            Err(source) => return Err(io_error(&document.absolute, source)),
        };
        Ok(Some(PreparedUpdate {
            id: document.id,
            generation,
            change,
        }))
    }

    pub fn apply_prepared(&mut self, update: PreparedUpdate) -> UpdateOutcome {
        let source = self.id_indices[&update.id];
        let current = self.generations[source as usize];
        if current != update.generation {
            return UpdateOutcome::StaleGeneration { current };
        }
        let affected = match update.change {
            PreparedChange::Replace(extracted) => self.install(*extracted),
            PreparedChange::Remove => self.remove_node(source, &update.id),
        };
        self.finish_update(update.generation, affected)
    }

    pub fn overlay_source(&self, path: impl AsRef<Path>) -> Option<&Source> {
        let document = self.document(path.as_ref()).ok()?;
        let source = self.id_indices.get(&document.id)?;
        self.overlays.get(source).map(|overlay| &overlay.source)
    }

    fn install(&mut self, extracted: ExtractedNode) -> BTreeSet<u32> {
        let source = self.intern(&extracted.node.id);
        let mut affected = BTreeSet::from([source]);
        affected.extend(
            self.outgoing[source as usize]
                .iter()
                .map(|link| link.target),
        );
        if !self.present[source as usize] {
            affected.extend(self.incoming[source as usize].iter().copied());
        }
        self.remove_outgoing(source);

        let mut occurrences = Vec::with_capacity(extracted.references.len());
        for reference in &extracted.references {
            let target = self.intern(&reference.target);
            affected.insert(target);
            occurrences.push((target, reference.range));
        }
        occurrences.sort();

        let mut outgoing = Vec::<OutgoingLink>::new();
        let mut spans = Vec::with_capacity(occurrences.len());
        for (target, span) in occurrences {
            let new_target = outgoing.last().is_none_or(|link| link.target != target);
            if new_target {
                outgoing.push(OutgoingLink {
                    target,
                    span_start: spans.len() as u32,
                    span_len: 0,
                });
                insert_sorted(&mut self.incoming[target as usize], source);
            }
            spans.push(span);
            outgoing.last_mut().expect("outgoing link exists").span_len += 1;
        }

        self.outgoing[source as usize] = outgoing;
        self.spans[source as usize] = spans;
        self.source_diagnostics[source as usize] = extracted.diagnostics;
        self.present[source as usize] = true;
        match self
            .nodes
            .binary_search_by(|node| node.id.cmp(&extracted.node.id))
        {
            Ok(index) => self.nodes[index] = extracted.node,
            Err(index) => self.nodes.insert(index, extracted.node),
        }
        affected
    }

    fn remove_node(&mut self, source: u32, id: &str) -> BTreeSet<u32> {
        let mut affected = BTreeSet::from([source]);
        affected.extend(
            self.outgoing[source as usize]
                .iter()
                .map(|link| link.target),
        );
        affected.extend(self.incoming[source as usize].iter().copied());
        self.remove_outgoing(source);
        self.source_diagnostics[source as usize].clear();
        self.graph_diagnostics[source as usize].clear();
        self.present[source as usize] = false;
        if let Ok(index) = self.nodes.binary_search_by(|node| node.id.as_str().cmp(id)) {
            self.nodes.remove(index);
        }
        affected
    }

    fn remove_outgoing(&mut self, source: u32) {
        for link in &self.outgoing[source as usize] {
            remove_sorted(&mut self.incoming[link.target as usize], source);
        }
        self.outgoing[source as usize].clear();
        self.spans[source as usize].clear();
    }

    fn intern(&mut self, id: &str) -> u32 {
        if let Some(&index) = self.id_indices.get(id) {
            return index;
        }
        let index = self.ids.len() as u32;
        self.ids.push(id.to_owned());
        self.id_indices.insert(id.to_owned(), index);
        self.source_diagnostics.push(Vec::new());
        self.graph_diagnostics.push(Vec::new());
        self.present.push(false);
        self.outgoing.push(Vec::new());
        self.spans.push(Vec::new());
        self.incoming.push(Vec::new());
        self.generations.push(0);
        index
    }

    fn schedule(&mut self, source: u32) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("source generation overflow");
        self.generations[source as usize] = generation;
        generation
    }

    fn finish_update(&mut self, generation: u64, affected: BTreeSet<u32>) -> UpdateOutcome {
        self.revision = self
            .revision
            .checked_add(1)
            .filter(|revision| *revision <= MAX_JSON_INTEGER)
            .expect("JSON-safe graph revision exhausted");
        for source in affected {
            self.rebuild_graph_diagnostics(source);
        }
        self.rebuild_diagnostic_list();
        UpdateOutcome::Applied {
            generation,
            revision: self.revision,
        }
    }

    fn rebuild_all_graph_diagnostics(&mut self) {
        for source in 0..self.ids.len() as u32 {
            self.rebuild_graph_diagnostics(source);
        }
    }

    fn rebuild_graph_diagnostics(&mut self, source: u32) {
        if !self.present[source as usize] {
            self.graph_diagnostics[source as usize].clear();
            return;
        }
        let path = self
            .node(&self.ids[source as usize])
            .expect("present IDs have nodes")
            .path
            .clone();
        let mut diagnostics = Vec::new();
        for link in &self.outgoing[source as usize] {
            if self.present[link.target as usize] {
                continue;
            }
            let start = link.span_start as usize;
            let end = start + link.span_len as usize;
            for span in &self.spans[source as usize][start..end] {
                diagnostics.push(Diagnostic {
                    path: path.clone(),
                    code: "reference.dangling".to_owned(),
                    severity: Severity::Error,
                    message: format!(
                        "reference target `{}` does not exist",
                        self.ids[link.target as usize]
                    ),
                    range: Some(*span),
                    field: None,
                });
            }
        }
        self.graph_diagnostics[source as usize] = diagnostics;
    }

    fn rebuild_diagnostic_list(&mut self) {
        let mut diagnostics = self.archive_diagnostics.clone();
        for node in &self.nodes {
            let source = self.id_indices[&node.id] as usize;
            diagnostics.extend(self.source_diagnostics[source].iter().cloned());
            diagnostics.extend(self.graph_diagnostics[source].iter().cloned());
        }
        diagnostics.sort_by(compare_diagnostics);
        self.diagnostics = diagnostics;
    }

    fn public_link(&self, source: u32, link: &OutgoingLink) -> Link {
        let start = link.span_start as usize;
        let end = start + link.span_len as usize;
        Link {
            source: self.ids[source as usize].clone(),
            target: self.ids[link.target as usize].clone(),
            resolution: if self.present[link.target as usize] {
                LinkResolution::Resolved
            } else {
                LinkResolution::Missing
            },
            spans: self.spans[source as usize][start..end].to_vec(),
        }
    }

    fn document(&self, path: &Path) -> Result<Document, ProviderError> {
        let relative = if path.is_absolute() {
            path.strip_prefix(&self.archive_root)
                .map_err(|_| ProviderError::NoncanonicalPath(path.to_path_buf()))?
        } else {
            path
        };
        let mut components = relative.components();
        let valid_parent =
            matches!(components.next(), Some(Component::Normal(part)) if part == "zettel");
        let Some(Component::Normal(filename)) = components.next() else {
            return Err(ProviderError::NoncanonicalPath(path.to_path_buf()));
        };
        if !valid_parent || components.next().is_some() {
            return Err(ProviderError::NoncanonicalPath(path.to_path_buf()));
        }
        let Some(id) = filename
            .to_str()
            .and_then(|name| name.strip_suffix(".typ"))
            .filter(|id| is_zettel_id(id))
        else {
            return Err(ProviderError::NoncanonicalPath(path.to_path_buf()));
        };
        let relative = Path::new("zettel").join(filename);
        Ok(Document {
            id: id.to_owned(),
            relative: relative
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/"),
            absolute: self.archive_root.join(relative),
        })
    }
}

#[derive(Debug)]
struct Document {
    id: String,
    relative: String,
    absolute: PathBuf,
}

struct DiscoveredPaths {
    canonical: Vec<(String, PathBuf)>,
    diagnostics: Vec<Diagnostic>,
}

fn discover_paths(root: &Path) -> Result<DiscoveredPaths, ProviderError> {
    let directory = root.join("zettel");
    let entries = fs::read_dir(&directory).map_err(|source| io_error(&directory, source))?;
    let mut canonical = Vec::new();
    let mut diagnostics = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| io_error(&directory, source))?;
        let path = entry.path();
        let relative = archive_relative(root, &path);
        let file_type = entry
            .file_type()
            .map_err(|source| io_error(&path, source))?;
        if !file_type.is_file() {
            diagnostics.push(Diagnostic {
                path: relative,
                code: "archive.layout".to_owned(),
                severity: Severity::Error,
                message: "only regular Zettel files are allowed in `zettel/`".to_owned(),
                range: None,
                field: None,
            });
            continue;
        }
        let id = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".typ"))
            .filter(|id| is_zettel_id(id));
        match id {
            Some(id) => canonical.push((id.to_owned(), path)),
            None => diagnostics.push(Diagnostic {
                path: relative,
                code: "archive.filename".to_owned(),
                severity: Severity::Error,
                message: "Zettel filename must match `YYMMDDHHmm.typ`".to_owned(),
                range: None,
                field: None,
            }),
        }
    }
    canonical.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(DiscoveredPaths {
        canonical,
        diagnostics,
    })
}

fn load_zettel(
    root: &Path,
    id: &str,
    path: &Path,
    contract: &MetadataContract,
) -> Result<ExtractedNode, ProviderError> {
    let source = fs::read_to_string(path).map_err(|source| io_error(path, source))?;
    let source = checked_source(path, source)?;
    Ok(extract(
        id,
        &archive_relative(root, path),
        &source,
        contract,
    ))
}

fn checked_source(path: &Path, source: String) -> Result<String, ProviderError> {
    ensure_source_size(path, &source)?;
    Ok(source)
}

fn ensure_source_size(path: &Path, source: &str) -> Result<(), ProviderError> {
    if u32::try_from(source.len()).is_err() {
        return Err(ProviderError::SourceTooLarge(path.to_path_buf()));
    }
    Ok(())
}

fn archive_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("archive paths are beneath the archive root")
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn node_matches(node: &ZettelNode, query: &str) -> bool {
    query.is_empty()
        || node.id.contains(query)
        || node
            .title
            .as_ref()
            .is_some_and(|title| title.text.to_lowercase().contains(query))
        || node
            .metadata
            .values()
            .flatten()
            .any(|value| value.matches_lowercase_query(query))
}

fn insert_sorted(values: &mut Vec<u32>, value: u32) {
    if let Err(index) = values.binary_search(&value) {
        values.insert(index, value);
    }
}

fn remove_sorted(values: &mut Vec<u32>, value: u32) {
    if let Ok(index) = values.binary_search(&value) {
        values.remove(index);
    }
}

fn io_error(path: &Path, source: std::io::Error) -> ProviderError {
    ProviderError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn zettel(id: &str, title: &str, body: &str) -> String {
        format!(
            r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= {title} <{id}>

#abstract[]

#keywords()

#category.thoughts

{body}
"#
        )
    }

    fn write_zettel(root: &Path, id: &str, title: &str, body: &str) -> PathBuf {
        let path = root.join("zettel").join(format!("{id}.typ"));
        fs::write(&path, zettel(id, title, body)).unwrap();
        path
    }

    #[test]
    fn groups_repeated_links_and_resolves_targets() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        write_zettel(
            archive.root(),
            "2603231410",
            "Source",
            "@2603231411 then @2603231411 and @9999999999",
        );
        write_zettel(
            archive.root(),
            "2603231411",
            "Target",
            "back to @2603231410",
        );

        let provider = Provider::load(&archive).unwrap();
        let snapshot = provider.snapshot();

        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(snapshot.links.len(), 3);
        let repeated = snapshot
            .links
            .iter()
            .find(|link| link.source == "2603231410" && link.target == "2603231411")
            .unwrap();
        assert_eq!(repeated.resolution, LinkResolution::Resolved);
        assert_eq!(repeated.spans.len(), 2);
        let missing = snapshot
            .links
            .iter()
            .find(|link| link.target == "9999999999")
            .unwrap();
        assert_eq!(missing.resolution, LinkResolution::Missing);
        let dangling = provider
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == "reference.dangling")
            .expect("dangling diagnostic");
        assert_eq!(dangling.path, "zettel/2603231410.typ");
        assert_eq!(dangling.range, Some(missing.spans[0]));

        assert_eq!(provider.links_from("2603231410").len(), 2);
        assert_eq!(provider.links_to("2603231411").len(), 1);
        assert_eq!(provider.links_to("9999999999").len(), 1);
    }

    #[test]
    fn diagnoses_noncanonical_files() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        write_zettel(archive.root(), "2603231410", "Note", "body");
        fs::write(archive.root().join("zettel/readme.typ"), "not a Zettel").unwrap();
        fs::write(archive.root().join("zettel/2603231410.txt"), "not Typst").unwrap();
        fs::write(
            archive.root().join("zettel/2602999999.typ"),
            "invalid timestamp",
        )
        .unwrap();

        let provider = Provider::load(&archive).unwrap();

        assert_eq!(provider.nodes().len(), 1);
        assert_eq!(provider.nodes()[0].id, "2603231410");
        assert_eq!(
            provider
                .diagnostics()
                .iter()
                .filter(|diagnostic| diagnostic.code == "archive.filename")
                .count(),
            3
        );
    }

    #[test]
    fn serializes_the_versioned_snapshot_shape() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        write_zettel(archive.root(), "2603231410", "Note", "body");
        let provider = Provider::load(&archive).unwrap();

        let value = serde_json::to_value(crate::model::Envelope::new(provider.snapshot())).unwrap();

        assert_eq!(value["schema_version"], 2);
        assert_eq!(value["data"]["revision"], 1);
        assert_eq!(value["data"]["nodes"][0]["id"], "2603231410");
        assert!(
            value["data"]["nodes"][0]["metadata"]
                .get("abstract")
                .is_some()
        );
        assert!(value["data"]["nodes"][0].get("generation").is_none());
        assert!(value["data"]["links"].is_array());
        assert!(value["data"]["diagnostics"].is_array());
    }

    #[test]
    fn buffer_changes_replace_links_and_reject_stale_versions() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        let source_path = write_zettel(archive.root(), "2603231410", "Disk", "link @2603231411");
        write_zettel(archive.root(), "2603231411", "Old target", "body");
        write_zettel(archive.root(), "2603231412", "New target", "body");
        let mut provider = Provider::load(&archive).unwrap();

        let opened = provider
            .open_buffer(
                &source_path,
                1,
                zettel("2603231410", "Open", "link @2603231411"),
            )
            .unwrap();
        assert!(matches!(opened, UpdateOutcome::Applied { revision: 2, .. }));
        assert_eq!(
            provider
                .node("2603231410")
                .unwrap()
                .title
                .as_ref()
                .unwrap()
                .text,
            "Open"
        );

        let changed_text = zettel("2603231410", "Changed", "link @2603231412");
        let changed = provider
            .change_buffer(&source_path, 2, &changed_text)
            .unwrap();
        let changed_generation = match changed {
            UpdateOutcome::Applied {
                generation,
                revision: 3,
            } => generation,
            other => panic!("expected applied change, got {other:?}"),
        };
        assert_eq!(
            provider.node("2603231410").unwrap().generation,
            changed_generation
        );
        assert_eq!(
            provider.overlay_source(&source_path).unwrap().text(),
            changed_text
        );
        assert!(provider.links_to("2603231411").is_empty());
        assert_eq!(provider.links_to("2603231412").len(), 1);
        assert!(provider.diagnostics().is_empty());

        let stale = provider
            .change_buffer(
                &source_path,
                2,
                &zettel("2603231410", "Stale", "link @2603231411"),
            )
            .unwrap();
        assert_eq!(stale, UpdateOutcome::StaleVersion { current: 2 });
        assert_eq!(provider.revision(), 3);
        assert_eq!(
            provider
                .node("2603231410")
                .unwrap()
                .title
                .as_ref()
                .unwrap()
                .text,
            "Changed"
        );
    }

    #[test]
    fn save_retains_overlay_and_close_reloads_disk() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        let path = write_zettel(archive.root(), "2603231410", "Disk", "body");
        let mut provider = Provider::load(&archive).unwrap();
        provider
            .open_buffer(&path, 1, zettel("2603231410", "Overlay", "body"))
            .unwrap();
        fs::write(&path, zettel("2603231410", "Saved disk", "body")).unwrap();

        let saved = provider.save_buffer(&path).unwrap();
        assert!(matches!(
            saved,
            UpdateOutcome::OverlayRetained { revision: 2, .. }
        ));
        assert_eq!(
            provider.refresh_disk(&path).unwrap(),
            UpdateOutcome::IgnoredOpenOverlay
        );
        assert_eq!(
            provider
                .node("2603231410")
                .unwrap()
                .title
                .as_ref()
                .unwrap()
                .text,
            "Overlay"
        );

        let closed = provider.close_buffer(&path).unwrap();
        assert!(matches!(closed, UpdateOutcome::Applied { revision: 3, .. }));
        assert_eq!(
            provider
                .node("2603231410")
                .unwrap()
                .title
                .as_ref()
                .unwrap()
                .text,
            "Saved disk"
        );
        assert!(provider.overlay_source(&path).is_none());
    }

    #[test]
    fn unsaved_buffer_creates_then_removes_a_session_node() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        let path = archive.root().join("zettel/2603231410.typ");
        let mut provider = Provider::load(&archive).unwrap();

        provider
            .open_buffer(&path, 1, zettel("2603231410", "Unsaved", "body"))
            .unwrap();
        assert!(provider.node("2603231410").is_some());
        assert_eq!(provider.nodes().len(), 1);

        provider.close_buffer(&path).unwrap();
        assert!(provider.node("2603231410").is_none());
        assert!(provider.nodes().is_empty());
    }

    #[test]
    fn stale_prepared_disk_update_cannot_replace_newer_state() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        let path = write_zettel(archive.root(), "2603231410", "Initial", "body");
        let mut provider = Provider::load(&archive).unwrap();

        fs::write(&path, zettel("2603231410", "Older", "body")).unwrap();
        let stale = provider.prepare_disk_update(&path).unwrap().unwrap();
        fs::write(&path, zettel("2603231410", "Newer", "body")).unwrap();
        let current = provider.prepare_disk_update(&path).unwrap().unwrap();
        let expected_generation = current.generation();

        let applied = provider.apply_prepared(current);
        assert_eq!(
            applied,
            UpdateOutcome::Applied {
                generation: expected_generation,
                revision: 2,
            }
        );
        let rejected = provider.apply_prepared(stale);
        assert_eq!(
            rejected,
            UpdateOutcome::StaleGeneration {
                current: expected_generation,
            }
        );
        assert_eq!(provider.revision(), 2);
        assert_eq!(
            provider
                .node("2603231410")
                .unwrap()
                .title
                .as_ref()
                .unwrap()
                .text,
            "Newer"
        );
    }

    #[test]
    fn prepared_disk_update_cannot_overwrite_a_new_overlay() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        let path = write_zettel(archive.root(), "2603231410", "Disk", "body");
        let mut provider = Provider::load(&archive).unwrap();

        fs::write(&path, zettel("2603231410", "Pending disk", "body")).unwrap();
        let pending = provider.prepare_disk_update(&path).unwrap().unwrap();
        let opened = provider
            .open_buffer(&path, 1, zettel("2603231410", "Overlay", "body"))
            .unwrap();
        let overlay_generation = match opened {
            UpdateOutcome::Applied { generation, .. } => generation,
            other => panic!("expected applied overlay, got {other:?}"),
        };

        assert_eq!(
            provider.apply_prepared(pending),
            UpdateOutcome::StaleGeneration {
                current: overlay_generation,
            }
        );
        assert_eq!(provider.revision(), 2);
        assert_eq!(
            provider
                .node("2603231410")
                .unwrap()
                .title
                .as_ref()
                .unwrap()
                .text,
            "Overlay"
        );
    }

    #[test]
    fn disk_refresh_removes_and_restores_target_resolution() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        write_zettel(archive.root(), "2603231410", "Source", "link @2603231411");
        let target = write_zettel(archive.root(), "2603231411", "Target", "body");
        let mut provider = Provider::load(&archive).unwrap();

        fs::remove_file(&target).unwrap();
        assert!(matches!(
            provider.refresh_disk(&target).unwrap(),
            UpdateOutcome::Applied { revision: 2, .. }
        ));
        assert!(provider.node("2603231411").is_none());
        assert_eq!(
            provider.links_from("2603231410")[0].resolution,
            LinkResolution::Missing
        );
        assert!(
            provider
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "reference.dangling")
        );

        fs::write(&target, zettel("2603231411", "Restored", "body")).unwrap();
        assert!(matches!(
            provider.refresh_disk(&target).unwrap(),
            UpdateOutcome::Applied { revision: 3, .. }
        ));
        assert_eq!(
            provider.links_from("2603231410")[0].resolution,
            LinkResolution::Resolved
        );
        assert!(
            !provider
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "reference.dangling")
        );
    }
}
