use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use thiserror::Error;

use crate::archive::{Archive, ArchiveError};
use crate::extract::{ExtractedNode, extract};
use crate::model::{
    ByteRange, Diagnostic, GraphSnapshot, Link, LinkResolution, PROVIDER_SCHEMA_VERSION, ZettelNode,
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
}

#[derive(Debug, Clone)]
struct LinkRecord {
    source: u32,
    target: u32,
    resolution: LinkResolution,
    span_start: u32,
    span_len: u32,
}

#[derive(Debug, Clone)]
pub struct Provider {
    revision: u64,
    nodes: Vec<ZettelNode>,
    diagnostics: Vec<Diagnostic>,
    ids: Vec<String>,
    links: Vec<LinkRecord>,
    spans: Vec<ByteRange>,
    outgoing: Vec<Vec<u32>>,
    incoming: Vec<Vec<u32>>,
}

impl Provider {
    pub fn load(archive: &Archive) -> Result<Self, ProviderError> {
        archive.validate_layout()?;
        let paths = canonical_paths(archive.root())?;
        let extracted: Result<Vec<_>, _> = paths
            .par_iter()
            .map(|(id, path)| load_zettel(archive.root(), id, path))
            .collect();
        Ok(Self::from_extracted(extracted?))
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

    pub fn links_from(&self, id: &str) -> Vec<Link> {
        self.link_indices(id, &self.outgoing)
            .map(|indices| {
                indices
                    .iter()
                    .map(|index| self.public_link(*index))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn links_to(&self, id: &str) -> Vec<Link> {
        self.link_indices(id, &self.incoming)
            .map(|indices| {
                indices
                    .iter()
                    .map(|index| self.public_link(*index))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn snapshot(&self) -> GraphSnapshot {
        GraphSnapshot {
            schema_version: PROVIDER_SCHEMA_VERSION,
            revision: self.revision,
            nodes: self.nodes.clone(),
            links: (0..self.links.len())
                .map(|index| self.public_link(index as u32))
                .collect(),
            diagnostics: self.diagnostics.clone(),
        }
    }

    fn from_extracted(extracted: Vec<ExtractedNode>) -> Self {
        let node_ids: BTreeSet<_> = extracted
            .iter()
            .map(|extracted| extracted.node.id.as_str())
            .collect();
        let ids: Vec<_> = node_ids
            .iter()
            .copied()
            .chain(extracted.iter().flat_map(|node| {
                node.references
                    .iter()
                    .map(|reference| reference.target.as_str())
            }))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(str::to_owned)
            .collect();
        let id_indices: HashMap<_, _> = ids
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index as u32))
            .collect();

        let occurrence_count = extracted.iter().map(|node| node.references.len()).sum();
        let mut occurrences = Vec::with_capacity(occurrence_count);
        for extracted_node in &extracted {
            let source = id_indices[extracted_node.node.id.as_str()];
            for reference in &extracted_node.references {
                occurrences.push((
                    source,
                    id_indices[reference.target.as_str()],
                    reference.range,
                ));
            }
        }
        occurrences.sort();

        let mut outgoing = vec![Vec::new(); ids.len()];
        let mut incoming = vec![Vec::new(); ids.len()];
        let mut links = Vec::new();
        let mut spans = Vec::with_capacity(occurrences.len());
        for (source, target, span) in occurrences {
            let is_new_link = links
                .last()
                .is_none_or(|link: &LinkRecord| link.source != source || link.target != target);
            if is_new_link {
                let index = links.len() as u32;
                links.push(LinkRecord {
                    source,
                    target,
                    resolution: if node_ids.contains(ids[target as usize].as_str()) {
                        LinkResolution::Resolved
                    } else {
                        LinkResolution::Missing
                    },
                    span_start: spans.len() as u32,
                    span_len: 0,
                });
                outgoing[source as usize].push(index);
                incoming[target as usize].push(index);
            }
            spans.push(span);
            links.last_mut().expect("link exists").span_len += 1;
        }

        let mut nodes = Vec::with_capacity(extracted.len());
        let mut diagnostics = Vec::new();
        for extracted_node in extracted {
            nodes.push(extracted_node.node);
            diagnostics.extend(extracted_node.diagnostics);
        }
        diagnostics.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.range.cmp(&right.range))
                .then_with(|| left.code.cmp(&right.code))
        });

        Self {
            revision: 1,
            nodes,
            diagnostics,
            ids,
            links,
            spans,
            outgoing,
            incoming,
        }
    }

    fn link_indices<'a>(&self, id: &str, index: &'a [Vec<u32>]) -> Option<&'a [u32]> {
        let id = self
            .ids
            .binary_search_by(|candidate| candidate.as_str().cmp(id))
            .ok()?;
        Some(&index[id])
    }

    fn public_link(&self, index: u32) -> Link {
        let link = &self.links[index as usize];
        let start = link.span_start as usize;
        let end = start + link.span_len as usize;
        Link {
            source: self.ids[link.source as usize].clone(),
            target: self.ids[link.target as usize].clone(),
            resolution: link.resolution,
            spans: self.spans[start..end].to_vec(),
        }
    }
}

fn canonical_paths(root: &Path) -> Result<Vec<(String, PathBuf)>, ProviderError> {
    let directory = root.join("zettel");
    let entries = fs::read_dir(&directory).map_err(|source| io_error(&directory, source))?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| io_error(&directory, source))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| io_error(&path, source))?;
        if !file_type.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(id) = name.strip_suffix(".typ") else {
            continue;
        };
        if id.len() != 10 || !id.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        paths.push((id.to_owned(), path));
    }
    paths.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(paths)
}

fn load_zettel(root: &Path, id: &str, path: &Path) -> Result<ExtractedNode, ProviderError> {
    let source = fs::read_to_string(path).map_err(|source| io_error(path, source))?;
    if u32::try_from(source.len()).is_err() {
        return Err(ProviderError::SourceTooLarge(path.to_path_buf()));
    }
    let relative = path
        .strip_prefix(root)
        .expect("canonical paths are beneath the archive root");
    let relative = relative
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    Ok(extract(id, &relative, &source))
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

    fn write_zettel(root: &Path, id: &str, body: &str) {
        let source = format!(
            r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Note {id} <{id}>

#abstract[]

#keywords()

#category.thoughts

{body}
"#
        );
        fs::write(root.join("zettel").join(format!("{id}.typ")), source).unwrap();
    }

    #[test]
    fn groups_repeated_links_and_resolves_targets() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        write_zettel(
            archive.root(),
            "2603231410",
            "@2603231411 then @2603231411 and @9999999999",
        );
        write_zettel(archive.root(), "2603231411", "back to @2603231410");

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

        assert_eq!(provider.links_from("2603231410").len(), 2);
        assert_eq!(provider.links_to("2603231411").len(), 1);
        assert_eq!(provider.links_to("9999999999").len(), 1);
    }

    #[test]
    fn ignores_noncanonical_files() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        write_zettel(archive.root(), "2603231410", "body");
        fs::write(archive.root().join("zettel/readme.typ"), "not a Zettel").unwrap();
        fs::write(archive.root().join("zettel/2603231410.txt"), "not Typst").unwrap();

        let provider = Provider::load(&archive).unwrap();

        assert_eq!(provider.nodes().len(), 1);
        assert_eq!(provider.nodes()[0].id, "2603231410");
    }

    #[test]
    fn serializes_the_versioned_snapshot_shape() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        write_zettel(archive.root(), "2603231410", "body");
        let provider = Provider::load(&archive).unwrap();

        let value = serde_json::to_value(provider.snapshot()).unwrap();

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["revision"], 1);
        assert_eq!(value["nodes"][0]["id"], "2603231410");
        assert!(value["nodes"][0].get("abstract").is_some());
        assert!(value["links"].is_array());
        assert!(value["diagnostics"].is_array());
    }
}
