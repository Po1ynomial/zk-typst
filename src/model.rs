use serde::Serialize;

pub const PROVIDER_SCHEMA_VERSION: u32 = 1;

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
pub struct ZettelNode {
    pub id: String,
    pub path: String,
    pub generation: u64,
    pub title: Option<MarkupValue>,
    #[serde(rename = "abstract")]
    pub abstract_value: Option<MarkupValue>,
    pub keywords: Option<Vec<String>>,
    pub category: Option<String>,
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
    pub schema_version: u32,
    pub revision: u64,
    pub nodes: Vec<ZettelNode>,
    pub links: Vec<Link>,
    pub diagnostics: Vec<Diagnostic>,
}
