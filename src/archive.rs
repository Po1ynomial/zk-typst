use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime};
use thiserror::Error;

use crate::config::{ARCHIVE_FORMAT, DEFAULT_TEMPLATE_PATH, Manifest, MetadataContract};
use crate::extract::extract;
use crate::model::Severity;
use crate::templates;

const MANIFEST_NAME: &str = "zk.toml";
const ZETTEL_DIR: &str = "zettel";
const LIBRARY_PATH: &str = "lib/zettel.typ";
const AGENT_SKILLS_DIR: &str = ".agents/skills";

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("no archive found at or above {0}")]
    NotFound(PathBuf),

    #[error("an archive already exists at {0}")]
    AlreadyInitialized(PathBuf),

    #[error("archive path already exists and cannot be initialized: {0}")]
    PathConflict(PathBuf),

    #[error("cannot parse archive manifest {path}: {source}")]
    InvalidManifest {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("archive format {found} is not supported; expected {ARCHIVE_FORMAT}")]
    UnsupportedFormat { found: u32 },

    #[error("invalid archive configuration {path}: {message}")]
    InvalidConfig { path: PathBuf, message: String },

    #[error("invalid Zettel template {path}: {message}")]
    InvalidTemplate { path: PathBuf, message: String },

    #[error("archive is missing required {kind}: {path}")]
    MissingLayout { kind: &'static str, path: PathBuf },

    #[error("cannot access {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("no Zettel ID is available in the supported century")]
    IdSpaceExhausted,

    #[error("invalid Zettel ID `{0}`; expected a valid `YYMMDDHHmm` timestamp")]
    InvalidId(String),
}

#[derive(Debug, Error)]
pub enum AgentSkillInstallWarning {
    #[error("agent skill `{name}` already exists at {path}; leaving it unchanged")]
    AlreadyExists { name: &'static str, path: PathBuf },

    #[error("cannot install agent skill `{name}` at {path}: {source}")]
    Io {
        name: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    root: PathBuf,
    manifest: Manifest,
}

impl Archive {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ArchiveError> {
        let root = root.as_ref();
        let manifest = root.join(MANIFEST_NAME);
        if !manifest.is_file() {
            return Err(ArchiveError::NotFound(root.to_path_buf()));
        }
        let manifest = read_manifest(&manifest)?;
        let archive = Self {
            root: root.to_path_buf(),
            manifest,
        };
        archive.validate_layout()?;
        Ok(archive)
    }

    pub fn discover(start: impl AsRef<Path>) -> Result<Self, ArchiveError> {
        let start = start.as_ref();
        let directory = if start.is_file() {
            start.parent().unwrap_or(start)
        } else {
            start
        };

        for candidate in directory.ancestors() {
            let manifest = candidate.join(MANIFEST_NAME);
            if manifest.is_file() {
                return Self::open(candidate);
            }
        }

        Err(ArchiveError::NotFound(start.to_path_buf()))
    }

    pub fn init(root: impl AsRef<Path>) -> Result<Self, ArchiveError> {
        let root = root.as_ref();
        fs::create_dir_all(root).map_err(|source| io_error(root, source))?;

        let manifest = root.join(MANIFEST_NAME);
        if manifest.exists() {
            return Err(ArchiveError::AlreadyInitialized(manifest));
        }

        let zettel_dir = root.join(ZETTEL_DIR);
        let library = root.join(LIBRARY_PATH);
        let template = root.join(DEFAULT_TEMPLATE_PATH);
        for path in [&zettel_dir, &library, &template] {
            if path.exists() {
                return Err(ArchiveError::PathConflict(path.to_path_buf()));
            }
        }

        fs::create_dir(&zettel_dir).map_err(|source| io_error(&zettel_dir, source))?;
        let library_dir = library.parent().expect("library path has a parent");
        fs::create_dir_all(library_dir).map_err(|source| io_error(library_dir, source))?;
        let template_dir = template.parent().expect("template path has a parent");
        fs::create_dir_all(template_dir).map_err(|source| io_error(template_dir, source))?;
        write_new(&manifest, templates::MANIFEST)?;
        write_new(&library, templates::LIBRARY)?;
        write_new(&template, templates::ZETTEL)?;

        Self::open(root)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn metadata_contract(&self) -> &MetadataContract {
        &self.manifest.metadata
    }

    pub fn validate_layout(&self) -> Result<(), ArchiveError> {
        let zettel_dir = self.root.join(ZETTEL_DIR);
        if !zettel_dir.is_dir() {
            return Err(ArchiveError::MissingLayout {
                kind: "Zettel directory",
                path: zettel_dir,
            });
        }

        Ok(())
    }

    pub fn create_zettel(&self) -> Result<PathBuf, ArchiveError> {
        self.create_zettel_at(Local::now().naive_local())
    }

    pub fn install_agent_skills(&self) -> Vec<AgentSkillInstallWarning> {
        templates::AGENT_SKILLS
            .iter()
            .filter_map(|skill| self.install_agent_skill(skill).err())
            .collect()
    }

    pub fn remove_zettel(&self, id: &str) -> Result<PathBuf, ArchiveError> {
        if !is_zettel_id(id) {
            return Err(ArchiveError::InvalidId(id.to_owned()));
        }
        let path = self.root.join(ZETTEL_DIR).join(format!("{id}.typ"));
        fs::remove_file(&path).map_err(|source| io_error(&path, source))?;
        Ok(path)
    }

    fn create_zettel_at(&self, start: NaiveDateTime) -> Result<PathBuf, ArchiveError> {
        self.validate_layout()?;
        let template_path = self.root.join(&self.manifest.new.template);
        let resolved =
            fs::canonicalize(&template_path).map_err(|source| io_error(&template_path, source))?;
        let root = fs::canonicalize(&self.root).map_err(|source| io_error(&self.root, source))?;
        if !resolved.starts_with(&root) {
            return Err(ArchiveError::InvalidTemplate {
                path: template_path,
                message: "template must resolve beneath the archive root".to_owned(),
            });
        }
        let template =
            fs::read_to_string(&resolved).map_err(|source| io_error(&template_path, source))?;
        if !template.contains("{{id}}") {
            return Err(ArchiveError::InvalidTemplate {
                path: template_path,
                message: "template must contain the {{id}} placeholder".to_owned(),
            });
        }
        let mut candidate = start;
        let century = candidate.year().div_euclid(100);

        loop {
            let id = candidate.format("%y%m%d%H%M").to_string();
            let path = self.root.join(ZETTEL_DIR).join(format!("{id}.typ"));
            let rendered = template.replace("{{id}}", &id);
            if u32::try_from(rendered.len()).is_err() {
                return Err(ArchiveError::InvalidTemplate {
                    path: template_path,
                    message: "rendered template exceeds the 4 GiB byte-range limit".to_owned(),
                });
            }
            let extracted = extract(&id, "", &rendered, self.metadata_contract());
            let errors: Vec<_> = extracted
                .diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.severity == Severity::Error
                        && diagnostic.code.starts_with("metadata.")
                })
                .map(|diagnostic| diagnostic.message.as_str())
                .collect();
            if !errors.is_empty() {
                return Err(ArchiveError::InvalidTemplate {
                    path: template_path,
                    message: errors.join("; "),
                });
            }
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(rendered.as_bytes())
                        .map_err(|source| io_error(&path, source))?;
                    return Ok(path);
                }
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                    candidate += Duration::minutes(1);
                    if candidate.year().div_euclid(100) == century {
                        continue;
                    }
                    return Err(ArchiveError::IdSpaceExhausted);
                }
                Err(source) => return Err(io_error(&path, source)),
            }
        }
    }

    fn install_agent_skill(
        &self,
        skill: &templates::AgentSkill,
    ) -> Result<(), AgentSkillInstallWarning> {
        let skills_dir = self.root.join(AGENT_SKILLS_DIR);
        fs::create_dir_all(&skills_dir).map_err(|source| AgentSkillInstallWarning::Io {
            name: skill.name,
            path: skills_dir.clone(),
            source,
        })?;

        let skill_dir = skills_dir.join(skill.name);
        match fs::create_dir(&skill_dir) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(AgentSkillInstallWarning::AlreadyExists {
                    name: skill.name,
                    path: skill_dir,
                });
            }
            Err(source) => {
                return Err(AgentSkillInstallWarning::Io {
                    name: skill.name,
                    path: skill_dir,
                    source,
                });
            }
        }

        let path = skill_dir.join("SKILL.md");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|source| AgentSkillInstallWarning::Io {
                name: skill.name,
                path: path.clone(),
                source,
            })?;
        file.write_all(skill.source.as_bytes())
            .map_err(|source| AgentSkillInstallWarning::Io {
                name: skill.name,
                path,
                source,
            })
    }
}

pub fn is_zettel_id(id: &str) -> bool {
    if id.len() != 10 || !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let year = 2000 + id[0..2].parse::<i32>().expect("digits checked");
    let month = id[2..4].parse::<u32>().expect("digits checked");
    let day = id[4..6].parse::<u32>().expect("digits checked");
    let hour = id[6..8].parse::<u32>().expect("digits checked");
    let minute = id[8..10].parse::<u32>().expect("digits checked");
    NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|date| date.and_hms_opt(hour, minute, 0))
        .is_some()
}

fn read_manifest(path: &Path) -> Result<Manifest, ArchiveError> {
    let source = fs::read_to_string(path).map_err(|source| io_error(path, source))?;
    let manifest: Manifest =
        toml::from_str(&source).map_err(|source| ArchiveError::InvalidManifest {
            path: path.to_path_buf(),
            source,
        })?;

    if manifest.format != ARCHIVE_FORMAT {
        return Err(ArchiveError::UnsupportedFormat {
            found: manifest.format,
        });
    }

    manifest
        .validate()
        .map_err(|message| ArchiveError::InvalidConfig {
            path: path.to_path_buf(),
            message,
        })?;
    Ok(manifest)
}

fn write_new(path: &Path, contents: &str) -> Result<(), ArchiveError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error(path, source))?;
    file.write_all(contents.as_bytes())
        .map_err(|source| io_error(path, source))
}

fn io_error(path: &Path, source: std::io::Error) -> ArchiveError {
    ArchiveError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn validates_timestamp_ids() {
        assert!(is_zettel_id("2603231410"));
        assert!(is_zettel_id("2402292359"));
        assert!(!is_zettel_id("2302292359"));
        assert!(!is_zettel_id("2613322460"));
        assert!(!is_zettel_id("not-an-id"));
    }

    #[test]
    fn initializes_the_fixed_layout() {
        let temporary = tempdir().unwrap();
        let root = temporary.path().join("notes");

        let archive = Archive::init(&root).unwrap();

        assert_eq!(archive.root(), root);
        assert_eq!(
            fs::read_to_string(root.join("zk.toml")).unwrap(),
            templates::MANIFEST
        );
        assert_eq!(
            fs::read_to_string(root.join(DEFAULT_TEMPLATE_PATH)).unwrap(),
            templates::ZETTEL
        );
        assert!(root.join("zettel").is_dir());
        assert_eq!(
            fs::read_to_string(root.join("lib/zettel.typ")).unwrap(),
            templates::LIBRARY
        );
        assert!(!root.join(".agents").exists());
    }

    #[test]
    fn installs_bundled_agent_skills_without_overwriting_existing_skills() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();

        assert!(archive.install_agent_skills().is_empty());
        let skill_path = temporary
            .path()
            .join(".agents/skills/zettelkasten/SKILL.md");
        assert_eq!(
            fs::read_to_string(&skill_path).unwrap(),
            templates::AGENT_SKILLS[0].source
        );

        fs::write(&skill_path, "user-owned\n").unwrap();
        let warnings = archive.install_agent_skills();

        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            &warnings[0],
            AgentSkillInstallWarning::AlreadyExists {
                name: "zettelkasten",
                ..
            }
        ));
        assert_eq!(fs::read_to_string(skill_path).unwrap(), "user-owned\n");
    }

    #[test]
    fn refuses_to_overwrite_an_existing_archive() {
        let temporary = tempdir().unwrap();
        fs::write(temporary.path().join("zk.toml"), "keep me").unwrap();

        let error = Archive::init(temporary.path()).unwrap_err();

        assert!(matches!(error, ArchiveError::AlreadyInitialized(_)));
        assert_eq!(
            fs::read_to_string(temporary.path().join("zk.toml")).unwrap(),
            "keep me"
        );
    }

    #[test]
    fn discovers_an_archive_from_a_nested_directory() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        let nested = temporary.path().join("assets/images");
        fs::create_dir_all(&nested).unwrap();

        assert_eq!(Archive::discover(nested).unwrap(), archive);
    }

    #[test]
    fn opens_only_an_exact_valid_archive_root() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        let nested = temporary.path().join("assets");
        fs::create_dir(&nested).unwrap();

        assert_eq!(Archive::open(temporary.path()).unwrap(), archive);
        assert!(matches!(
            Archive::open(&nested).unwrap_err(),
            ArchiveError::NotFound(path) if path == nested
        ));
    }

    #[test]
    fn opening_an_archive_validates_its_layout() {
        let temporary = tempdir().unwrap();
        fs::write(temporary.path().join("zk.toml"), "format = 2\n").unwrap();

        assert!(matches!(
            Archive::open(temporary.path()).unwrap_err(),
            ArchiveError::MissingLayout { .. }
        ));
    }

    #[test]
    fn rejects_an_unsupported_archive_format() {
        let temporary = tempdir().unwrap();
        fs::write(temporary.path().join("zk.toml"), "format = 1\n").unwrap();

        let error = Archive::discover(temporary.path()).unwrap_err();

        assert!(matches!(
            error,
            ArchiveError::UnsupportedFormat { found: 1 }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_templates_that_escape_through_a_symlink() {
        let temporary = tempdir().unwrap();
        let external = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        let target = external.path().join("external.tpl");
        fs::write(&target, "= Title <{{id}}>\n").unwrap();
        let template = archive.root().join(DEFAULT_TEMPLATE_PATH);
        fs::remove_file(&template).unwrap();
        std::os::unix::fs::symlink(&target, &template).unwrap();
        assert!(matches!(
            archive.create_zettel(),
            Err(ArchiveError::InvalidTemplate { .. })
        ));
        assert_eq!(
            fs::read_dir(archive.root().join("zettel")).unwrap().count(),
            0
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "= Title <{{id}}>\n");
    }

    #[test]
    fn advances_one_minute_when_an_id_is_occupied() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        let start = NaiveDate::from_ymd_opt(2026, 3, 23)
            .unwrap()
            .and_hms_opt(14, 10, 0)
            .unwrap();
        fs::write(temporary.path().join("zettel/2603231410.typ"), "existing").unwrap();

        let path = archive.create_zettel_at(start).unwrap();

        assert_eq!(path, temporary.path().join("zettel/2603231411.typ"));
        assert!(
            fs::read_to_string(path)
                .unwrap()
                .contains("= Untitled <2603231411>")
        );
    }

    #[test]
    fn allocation_can_cross_a_year_boundary() {
        let temporary = tempdir().unwrap();
        let archive = Archive::init(temporary.path()).unwrap();
        let start = NaiveDate::from_ymd_opt(2026, 12, 31)
            .unwrap()
            .and_hms_opt(23, 59, 0)
            .unwrap();
        fs::write(temporary.path().join("zettel/2612312359.typ"), "existing").unwrap();

        let path = archive.create_zettel_at(start).unwrap();

        assert_eq!(path, temporary.path().join("zettel/2701010000.typ"));
    }
}
