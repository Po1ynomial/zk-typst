use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration, Local, NaiveDateTime};
use serde::Deserialize;
use thiserror::Error;

use crate::templates;

const MANIFEST_NAME: &str = "zk.toml";
const ZETTEL_DIR: &str = "zettel";
const LIBRARY_PATH: &str = "lib/zettel.typ";
const ARCHIVE_FORMAT: u32 = 1;

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

    #[error("archive is missing required {kind}: {path}")]
    MissingLayout { kind: &'static str, path: PathBuf },

    #[error("cannot access {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("no Zettel ID is available in the supported century")]
    IdSpaceExhausted,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    root: PathBuf,
}

impl Archive {
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
                validate_manifest(&manifest)?;
                return Ok(Self {
                    root: candidate.to_path_buf(),
                });
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
        for path in [&zettel_dir, &library] {
            if path.exists() {
                return Err(ArchiveError::PathConflict(path.to_path_buf()));
            }
        }

        fs::create_dir(&zettel_dir).map_err(|source| io_error(&zettel_dir, source))?;
        let library_dir = library.parent().expect("library path has a parent");
        fs::create_dir(library_dir).map_err(|source| io_error(library_dir, source))?;
        write_new(&manifest, templates::MANIFEST)?;
        write_new(&library, templates::LIBRARY)?;

        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn validate_layout(&self) -> Result<(), ArchiveError> {
        let zettel_dir = self.root.join(ZETTEL_DIR);
        if !zettel_dir.is_dir() {
            return Err(ArchiveError::MissingLayout {
                kind: "Zettel directory",
                path: zettel_dir,
            });
        }

        let library = self.root.join(LIBRARY_PATH);
        if !library.is_file() {
            return Err(ArchiveError::MissingLayout {
                kind: "Typst library",
                path: library,
            });
        }

        Ok(())
    }

    pub fn create_zettel(&self) -> Result<PathBuf, ArchiveError> {
        self.create_zettel_at(Local::now().naive_local())
    }

    fn create_zettel_at(&self, start: NaiveDateTime) -> Result<PathBuf, ArchiveError> {
        self.validate_layout()?;
        let mut candidate = start;
        let century = candidate.year().div_euclid(100);

        loop {
            let id = candidate.format("%y%m%d%H%M").to_string();
            let path = self.root.join(ZETTEL_DIR).join(format!("{id}.typ"));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(templates::zettel(&id).as_bytes())
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
}

fn validate_manifest(path: &Path) -> Result<(), ArchiveError> {
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

    Ok(())
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
    fn initializes_the_fixed_layout() {
        let temporary = tempdir().unwrap();
        let root = temporary.path().join("notes");

        let archive = Archive::init(&root).unwrap();

        assert_eq!(archive.root(), root);
        assert_eq!(
            fs::read_to_string(root.join("zk.toml")).unwrap(),
            "format = 1\n"
        );
        assert!(root.join("zettel").is_dir());
        assert_eq!(
            fs::read_to_string(root.join("lib/zettel.typ")).unwrap(),
            templates::LIBRARY
        );
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
    fn rejects_an_unsupported_archive_format() {
        let temporary = tempdir().unwrap();
        fs::write(temporary.path().join("zk.toml"), "format = 2\n").unwrap();

        let error = Archive::discover(temporary.path()).unwrap_err();

        assert!(matches!(
            error,
            ArchiveError::UnsupportedFormat { found: 2 }
        ));
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
