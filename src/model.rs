use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::Serialize;

pub const DATA_SCHEMA_VERSION: u32 = 2;
pub const MAX_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Envelope<T> {
    pub schema_version: u32,
    pub data: T,
}

impl<T> Envelope<T> {
    pub fn new(data: T) -> Self {
        Self {
            schema_version: DATA_SCHEMA_VERSION,
            data,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ByteRange {
    pub start: u32,
    pub end: u32,
}

impl ByteRange {
    pub(crate) fn from_usize(start: usize, end: usize) -> Option<Self> {
        Some(Self {
            start: start.try_into().ok()?,
            end: end.try_into().ok()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MarkupValue {
    pub source: String,
    pub text: String,
    pub range: ByteRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum MetadataValue {
    Markup(MarkupValue),
    String(String),
    StringList(Vec<String>),
}

impl MetadataValue {
    pub fn as_markup(&self) -> Option<&MarkupValue> {
        match self {
            Self::Markup(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn matches_lowercase_query(&self, query: &str) -> bool {
        match self {
            Self::Markup(value) => value.text.to_lowercase().contains(query),
            Self::String(value) => value.to_lowercase().contains(query),
            Self::StringList(values) => values
                .iter()
                .any(|value| value.to_lowercase().contains(query)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZettelNode {
    pub id: String,
    pub path: String,
    #[serde(skip_serializing)]
    pub generation: u64,
    pub title: Option<MarkupValue>,
    pub metadata: BTreeMap<String, Option<MetadataValue>>,
}

/// A stored file in a note-ID namespace, not a graph node or dependency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Asset {
    pub note_id: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub path: String,
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub range: Option<ByteRange>,
    pub field: Option<String>,
}

pub(crate) fn compare_diagnostics(left: &Diagnostic, right: &Diagnostic) -> Ordering {
    left.path
        .cmp(&right.path)
        .then_with(|| left.range.cmp(&right.range))
        .then_with(|| left.code.cmp(&right.code))
        .then_with(|| left.field.cmp(&right.field))
        .then_with(|| left.message.cmp(&right.message))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkResolution {
    Resolved,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    pub source: String,
    pub target: String,
    pub resolution: LinkResolution,
    pub spans: Vec<ByteRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphSnapshot {
    pub revision: u64,
    pub nodes: Vec<ZettelNode>,
    pub links: Vec<Link>,
    pub diagnostics: Vec<Diagnostic>,
}
