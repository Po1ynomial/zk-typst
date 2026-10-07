use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use tempfile::{Builder, NamedTempFile};
use thiserror::Error;

use crate::archive::{Archive, is_zettel_id};
use crate::model::Asset;

const ASSET_DIR: &str = "assets";

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("invalid Zettel ID `{0}`; expected a valid `YYMMDDHHmm` timestamp")]
    InvalidId(String),

    #[error("Zettel `{0}` does not exist")]
    MissingNote(String),

    #[error(
        "invalid asset name `{0}`; expected a nonempty UTF-8 relative path without traversal, backslashes, or control characters"
    )]
    InvalidName(String),

    #[error("source has no UTF-8 basename; supply --name: {0}")]
    SourceName(PathBuf),

    #[error("expected a regular file: {0}")]
    NotFile(PathBuf),

    #[error("expected a directory: {0}")]
    NotDirectory(PathBuf),

    #[error("asset commands do not traverse symlinks: {0}")]
    Symlink(PathBuf),

    #[error("asset destination already exists: {0}")]
    AlreadyExists(PathBuf),

    #[error("cannot copy {from} to {to}: {source}")]
    Copy {
        from: PathBuf,
        to: PathBuf,
        source: io::Error,
    },

    #[error("cannot access {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

impl Archive {
    /// Copy an opaque file into an existing saved note's namespace without clobbering.
    pub fn add_asset(
        &self,
        id: &str,
        source: &Path,
        name: Option<&str>,
    ) -> Result<Asset, AssetError> {
        validate_id(id)?;
        let root = canonical_root(self)?;
        let note_dir = directory(&root, Path::new("zettel"), false)?;
        let note = root.join("zettel").join(format!("{id}.typ"));
        if note_dir.is_none() {
            return Err(AssetError::MissingNote(id.to_owned()));
        }
        match fs::symlink_metadata(&note) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(AssetError::Symlink(note));
            }
            Ok(metadata) if !metadata.is_file() => return Err(AssetError::NotFile(note)),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(AssetError::MissingNote(id.to_owned()));
            }
            Err(source) => return Err(io_error(&note, source)),
        }
        let name = name.map(Ok).unwrap_or_else(|| {
            source
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| AssetError::SourceName(source.to_path_buf()))
        })?;
        validate_name(name)?;
        // Explicit source symlinks may resolve to a regular file; only bytes are copied.
        let metadata = fs::metadata(source).map_err(|error| io_error(source, error))?;
        if !metadata.is_file() {
            return Err(AssetError::NotFile(source.to_path_buf()));
        }
        let mut input = File::open(source).map_err(|error| io_error(source, error))?;
        if !input
            .metadata()
            .map_err(|error| io_error(source, error))?
            .is_file()
        {
            return Err(AssetError::NotFile(source.to_path_buf()));
        }
        let asset = asset(id, name);
        let relative = Path::new(&asset.path);
        let parent = directory(&root, relative.parent().expect("asset has a parent"), true)?
            .expect("created directory exists");
        let destination = parent.join(relative.file_name().expect("asset has a filename"));
        match fs::symlink_metadata(&destination) {
            Ok(_) => return Err(AssetError::AlreadyExists(destination)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(io_error(&destination, source)),
        }
        // Stage outside note namespaces so concurrent listings cannot expose partial uploads.
        let asset_root = root.join(ASSET_DIR);
        copy_and_publish(&mut input, source, &asset_root, &destination)?;
        Ok(asset)
    }

    /// List regular files recursively, including namespaces whose saved notes are absent.
    pub fn list_assets(&self, id: &str) -> Result<Vec<Asset>, AssetError> {
        validate_id(id)?;
        let root = canonical_root(self)?;
        let relative = PathBuf::from(ASSET_DIR).join(id);
        let Some(namespace) = directory(&root, &relative, false)? else {
            return Ok(Vec::new());
        };
        let mut pending = vec![namespace.clone()];
        let mut assets = Vec::new();
        while let Some(path) = pending.pop() {
            let entries = fs::read_dir(&path).map_err(|source| io_error(&path, source))?;
            for entry in entries {
                let entry = entry.map_err(|source| io_error(&path, source))?;
                let path = entry.path();
                let relative = path
                    .strip_prefix(&namespace)
                    .expect("entry belongs to namespace");
                let name = relative
                    .to_str()
                    .ok_or_else(|| {
                        AssetError::InvalidName(relative.to_string_lossy().into_owned())
                    })?
                    .replace(std::path::MAIN_SEPARATOR, "/");
                validate_name(&name)?;
                let kind = entry
                    .file_type()
                    .map_err(|source| io_error(&path, source))?;
                if kind.is_symlink() {
                    return Err(AssetError::Symlink(path));
                }
                if kind.is_dir() {
                    pending.push(path);
                } else if kind.is_file() {
                    assets.push(asset(id, &name));
                } else {
                    return Err(AssetError::NotFile(path));
                }
            }
        }
        assets.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(assets)
    }

    /// Remove exactly one regular file without interpreting its contents or references.
    pub fn remove_asset(&self, id: &str, name: &str) -> Result<Asset, AssetError> {
        validate_id(id)?;
        validate_name(name)?;
        let root = canonical_root(self)?;
        let asset = asset(id, name);
        let relative = Path::new(&asset.path);
        let destination = root.join(relative);
        if directory(&root, relative.parent().expect("asset has a parent"), false)?.is_none() {
            return Err(io_error(
                &destination,
                io::Error::new(io::ErrorKind::NotFound, "asset does not exist"),
            ));
        }
        let metadata =
            fs::symlink_metadata(&destination).map_err(|source| io_error(&destination, source))?;
        if metadata.file_type().is_symlink() {
            return Err(AssetError::Symlink(destination));
        }
        if !metadata.is_file() {
            return Err(AssetError::NotFile(destination));
        }
        fs::remove_file(&destination).map_err(|source| io_error(&destination, source))?;
        Ok(asset)
    }
}

fn asset(id: &str, name: &str) -> Asset {
    Asset {
        note_id: id.to_owned(),
        name: name.to_owned(),
        path: format!("{ASSET_DIR}/{id}/{name}"),
    }
}

fn validate_id(id: &str) -> Result<(), AssetError> {
    if is_zettel_id(id) {
        Ok(())
    } else {
        Err(AssetError::InvalidId(id.to_owned()))
    }
}

fn validate_name(name: &str) -> Result<(), AssetError> {
    if name.is_empty()
        || name.contains('\\')
        || name.chars().any(char::is_control)
        || name
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || !Path::new(name)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(AssetError::InvalidName(name.to_owned()));
    }
    Ok(())
}

fn canonical_root(archive: &Archive) -> Result<PathBuf, AssetError> {
    fs::canonicalize(archive.root()).map_err(|source| io_error(archive.root(), source))
}

/// Walk without following symlinks, optionally creating missing directories one at a time.
fn directory(root: &Path, relative: &Path, create: bool) -> Result<Option<PathBuf>, AssetError> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(AssetError::InvalidName(
                relative.to_string_lossy().into_owned(),
            ));
        };
        path.push(component);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
                match fs::create_dir(&path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(source) => return Err(io_error(&path, source)),
                }
                fs::symlink_metadata(&path).map_err(|source| io_error(&path, source))?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(io_error(&path, source)),
        };
        if metadata.file_type().is_symlink() {
            return Err(AssetError::Symlink(path));
        }
        if !metadata.is_dir() {
            return Err(AssetError::NotDirectory(path));
        }
    }
    Ok(Some(path))
}

fn copy_and_publish(
    input: &mut impl Read,
    source: &Path,
    asset_root: &Path,
    destination: &Path,
) -> Result<(), AssetError> {
    let staging = Builder::new()
        .prefix(".zk-asset-")
        .tempdir_in(asset_root)
        .map_err(|source| io_error(asset_root, source))?;
    let mut file =
        NamedTempFile::new_in(staging.path()).map_err(|source| io_error(staging.path(), source))?;
    io::copy(input, file.as_file_mut()).map_err(|error| AssetError::Copy {
        from: source.to_path_buf(),
        to: destination.to_path_buf(),
        source: error,
    })?;
    file.as_file()
        .sync_all()
        .map_err(|source| io_error(destination, source))?;
    file.persist_noclobber(destination).map_err(|error| {
        if error.error.kind() == io::ErrorKind::AlreadyExists {
            AssetError::AlreadyExists(destination.to_path_buf())
        } else {
            io_error(destination, error.error)
        }
    })?;
    Ok(())
}

fn io_error(path: &Path, source: io::Error) -> AssetError {
    AssetError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_copy_or_publication_leaves_no_partial_asset_or_staging_files() {
        struct BrokenReader(bool);
        impl Read for BrokenReader {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                if self.0 {
                    return Err(io::Error::other("simulated read failure"));
                }
                self.0 = true;
                buffer[0] = 42;
                Ok(1)
            }
        }
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination.bin");
        assert!(
            copy_and_publish(
                &mut BrokenReader(false),
                Path::new("source"),
                root.path(),
                &destination
            )
            .is_err()
        );
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
        fs::write(&destination, b"original").unwrap();
        assert!(matches!(
            copy_and_publish(
                &mut &b"replacement"[..],
                Path::new("source"),
                root.path(),
                &destination
            ),
            Err(AssetError::AlreadyExists(_))
        ));
        assert_eq!(fs::read(&destination).unwrap(), b"original");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
