use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::tempdir;
use zk::archive::Archive;

const ID: &str = "2603231410";

fn zk(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zk"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    output
}

fn failure(output: Output) {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

fn data(output: Output) -> serde_json::Value {
    let value: serde_json::Value = serde_json::from_slice(&success(output).stdout).unwrap();
    assert_eq!(value["schema_version"], 2);
    value["data"].clone()
}

fn note(root: &Path) {
    fs::write(
        root.join(format!("zettel/{ID}.typ")),
        format!("= Title <{ID}>\n"),
    )
    .unwrap();
}

#[test]
fn assets_are_lazy_opaque_and_listed_with_the_shared_schema_fixture() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    Archive::init(&root).unwrap();
    note(&root);
    assert!(!root.join("assets").exists());
    assert_eq!(
        data(zk(&root, &["asset", "list", ID])),
        serde_json::json!([])
    );
    assert!(!root.join("assets").exists());
    let before = data(zk(&root, &["graph", "--format", "json"]));
    let source = temporary.path().join("tiger.jpg");
    let bytes = [0, 0xff, 0x80, 10, 13];
    fs::write(&source, bytes).unwrap();
    let output = success(zk(&root, &["asset", "add", ID, source.to_str().unwrap()]));
    assert_eq!(output.stdout, format!("assets/{ID}/tiger.jpg\n").as_bytes());
    assert_eq!(
        fs::read(root.join(format!("assets/{ID}/tiger.jpg"))).unwrap(),
        bytes
    );
    assert_eq!(fs::read(&source).unwrap(), bytes);
    let helper = temporary.path().join("helper.typ");
    let helper_source = "#import \"../another.typ\": helper\nAn unindexed @9999999999.\n";
    fs::write(&helper, helper_source).unwrap();
    success(zk(
        &root,
        &[
            "asset",
            "add",
            ID,
            helper.to_str().unwrap(),
            "--name",
            "fragments/example.typ",
        ],
    ));
    assert_eq!(
        fs::read_to_string(root.join(format!("assets/{ID}/fragments/example.typ"))).unwrap(),
        helper_source
    );
    let actual: serde_json::Value =
        serde_json::from_slice(&success(zk(&root, &["asset", "list", ID])).stdout).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema2/assets.json")).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(data(zk(&root, &["graph", "--format", "json"])), before);
    success(zk(&root, &["check"]));
    assert_eq!(fs::read_dir(root.join("assets")).unwrap().count(), 1);
    assert!(!root.join(format!("assets/{ID}/another.typ")).exists());
}

#[test]
fn copying_never_clobbers_and_removal_is_exact() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    Archive::init(root).unwrap();
    note(root);
    let source = root.join("source.bin");
    fs::write(&source, b"original").unwrap();
    success(zk(
        root,
        &[
            "asset",
            "add",
            ID,
            "source.bin",
            "--name",
            "pictures/café.bin",
        ],
    ));
    fs::write(&source, b"replacement").unwrap();
    failure(zk(
        root,
        &[
            "asset",
            "add",
            ID,
            "source.bin",
            "--name",
            "pictures/café.bin",
        ],
    ));
    let destination = root.join(format!("assets/{ID}/pictures/café.bin"));
    assert_eq!(fs::read(&destination).unwrap(), b"original");
    failure(zk(root, &["asset", "remove", ID, "pictures"]));
    failure(zk(root, &["asset", "remove", ID, "pictures/*.bin"]));
    assert!(destination.exists());
    let output = success(zk(root, &["asset", "remove", ID, "pictures/café.bin"]));
    assert_eq!(
        output.stdout,
        format!("assets/{ID}/pictures/café.bin\n").as_bytes()
    );
    assert!(!destination.exists());
    assert_eq!(fs::read(&source).unwrap(), b"replacement");
    failure(zk(root, &["asset", "remove", ID, "pictures/café.bin"]));
    assert_eq!(
        data(zk(root, &["asset", "list", ID])),
        serde_json::json!([])
    );
    assert_eq!(fs::read_dir(root.join("assets")).unwrap().count(), 1);
}

#[test]
fn note_removal_retains_assets_and_orphan_namespaces_can_be_managed() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    Archive::init(root).unwrap();
    note(root);
    fs::write(root.join("data.bin"), b"asset").unwrap();
    success(zk(root, &["asset", "add", ID, "data.bin"]));
    success(zk(root, &["remove", ID]));
    assert!(root.join(format!("assets/{ID}/data.bin")).is_file());
    assert_eq!(
        data(zk(root, &["asset", "list", ID]))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    failure(zk(
        root,
        &["asset", "add", ID, "data.bin", "--name", "another.bin"],
    ));
    success(zk(root, &["asset", "remove", ID, "data.bin"]));
    assert_eq!(
        data(zk(root, &["asset", "list", ID])),
        serde_json::json!([])
    );
}

#[test]
fn invalid_ids_names_sources_and_destinations_do_not_create_files() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    Archive::init(root).unwrap();
    note(root);
    fs::write(root.join("source.bin"), b"source").unwrap();
    for id in ["bad", "9999999999", "../2603231410"] {
        failure(zk(root, &["asset", "add", id, "source.bin"]));
        failure(zk(root, &["asset", "list", id]));
        failure(zk(root, &["asset", "remove", id, "source.bin"]));
    }
    failure(zk(root, &["asset", "add", "2603231411", "source.bin"]));
    for name in [
        "",
        ".",
        "..",
        "../escape",
        "/absolute",
        "a/../escape",
        "a/./b",
        "a//b",
        "a/",
        "a\\b",
        "a\nb",
    ] {
        failure(zk(
            root,
            &["asset", "add", ID, "source.bin", "--name", name],
        ));
        failure(zk(root, &["asset", "remove", ID, name]));
    }
    failure(zk(root, &["asset", "add", ID, "absent.bin"]));
    failure(zk(root, &["asset", "add", ID, "zettel"]));
    assert!(!root.join("assets").exists());
    fs::write(root.join("assets"), b"not a directory").unwrap();
    failure(zk(root, &["asset", "add", ID, "source.bin"]));
    failure(zk(root, &["asset", "list", ID]));
    assert_eq!(fs::read(root.join("assets")).unwrap(), b"not a directory");
}

#[test]
fn assets_use_saved_filename_identity_without_loading_the_note_graph() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    Archive::init(root).unwrap();
    let malformed = "= Broken <wrong>\n#let incomplete = (\n";
    fs::write(root.join(format!("zettel/{ID}.typ")), malformed).unwrap();
    fs::write(root.join("zettel/2603231411.typ"), [0xff]).unwrap();
    fs::write(root.join("source.bin"), b"opaque").unwrap();
    success(zk(root, &["asset", "add", ID, "source.bin"]));
    assert_eq!(
        data(zk(root, &["asset", "list", ID]))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        fs::read_to_string(root.join(format!("zettel/{ID}.typ"))).unwrap(),
        malformed
    );
    success(zk(root, &["asset", "remove", ID, "source.bin"]));
}

#[test]
fn archive_selection_source_paths_and_listing_order_are_deterministic() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    let work = temporary.path().join("work");
    Archive::init(&root).unwrap();
    fs::create_dir(&work).unwrap();
    note(&root);
    fs::write(work.join("source.bin"), b"outside the archive").unwrap();
    for name in ["é.bin", "z.bin", "A.bin"] {
        success(zk(
            &work,
            &[
                "--archive",
                root.to_str().unwrap(),
                "asset",
                "add",
                ID,
                "source.bin",
                "--name",
                name,
            ],
        ));
    }
    fs::create_dir_all(root.join("assets/shared")).unwrap();
    fs::write(root.join("assets/shared/logo.svg"), b"unmanaged").unwrap();
    let values = data(zk(
        &work,
        &["--archive", root.to_str().unwrap(), "asset", "list", ID],
    ));
    assert_eq!(
        values
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["A.bin", "z.bin", "é.bin"]
    );
    fs::rename(&root, temporary.path().join("moved")).unwrap();
    let moved = temporary.path().join("moved");
    assert_eq!(data(zk(&moved, &["asset", "list", ID])), values);
    assert_eq!(
        fs::read(moved.join(format!("assets/{ID}/A.bin"))).unwrap(),
        b"outside the archive"
    );
}

#[cfg(unix)]
#[test]
fn commands_never_follow_destination_namespace_or_note_symlinks() {
    use std::os::unix::fs::symlink;
    for relative in ["assets", "assets/2603231410", "assets/2603231410/nested"] {
        let temporary = tempdir().unwrap();
        let external = tempdir().unwrap();
        let root = temporary.path();
        Archive::init(root).unwrap();
        note(root);
        fs::write(root.join("source.bin"), b"source").unwrap();
        let link = root.join(relative);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(external.path(), &link).unwrap();
        let name = if relative.ends_with("nested") {
            "nested/source.bin"
        } else {
            "source.bin"
        };
        failure(zk(
            root,
            &["asset", "add", ID, "source.bin", "--name", name],
        ));
        failure(zk(root, &["asset", "list", ID]));
        failure(zk(root, &["asset", "remove", ID, name]));
        assert_eq!(fs::read_dir(external.path()).unwrap().count(), 0);
    }
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    Archive::init(root).unwrap();
    fs::write(root.join("source.bin"), b"source").unwrap();
    symlink(
        root.join("source.bin"),
        root.join(format!("zettel/{ID}.typ")),
    )
    .unwrap();
    failure(zk(root, &["asset", "add", ID, "source.bin"]));
    assert!(!root.join("assets").exists());
}

#[cfg(unix)]
#[test]
fn existing_symlinks_are_not_clobbered_removed_or_listed_and_source_links_copy_bytes() {
    use std::os::unix::fs::symlink;
    let temporary = tempdir().unwrap();
    let external = tempdir().unwrap();
    let root = temporary.path();
    Archive::init(root).unwrap();
    note(root);
    let namespace = root.join(format!("assets/{ID}"));
    fs::create_dir_all(&namespace).unwrap();
    let target = external.path().join("target.bin");
    fs::write(&target, b"external").unwrap();
    for name in ["outside.bin", "dangling.bin"] {
        symlink(
            if name == "outside.bin" {
                target.clone()
            } else {
                external.path().join("missing.bin")
            },
            namespace.join(name),
        )
        .unwrap();
        failure(zk(
            root,
            &["asset", "add", ID, target.to_str().unwrap(), "--name", name],
        ));
        failure(zk(root, &["asset", "remove", ID, name]));
    }
    failure(zk(root, &["asset", "list", ID]));
    assert_eq!(fs::read(&target).unwrap(), b"external");
    assert!(!external.path().join("missing.bin").exists());
    let source_link = root.join("source-link.bin");
    symlink(&target, &source_link).unwrap();
    success(zk(root, &["asset", "add", ID, "source-link.bin"]));
    let copy = namespace.join("source-link.bin");
    assert!(fs::symlink_metadata(&copy).unwrap().is_file());
    assert_eq!(fs::read(&copy).unwrap(), b"external");
}

#[cfg(unix)]
#[test]
fn non_utf8_source_names_require_an_explicit_destination_name() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    let archive = Archive::init(root).unwrap();
    note(root);
    let source = root.join(OsStr::from_bytes(b"source-\xff.bin"));
    assert!(matches!(
        archive.add_asset(ID, &source, None),
        Err(zk::assets::AssetError::SourceName(_))
    ));
    assert!(!root.join("assets").exists());
    // An explicit UTF-8 name reaches file access rather than basename rejection.
    assert!(matches!(
        archive.add_asset(ID, &source, Some("valid.bin")),
        Err(zk::assets::AssetError::Io { .. })
    ));
}

// APFS rejects non-UTF-8 filenames before the command can inspect them.
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_files_can_be_ingested_with_a_name_but_cannot_be_listed_as_assets() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    let archive = Archive::init(root).unwrap();
    note(root);
    let source = root.join(OsStr::from_bytes(b"source-\xff.bin"));
    fs::write(&source, b"opaque").unwrap();
    archive.add_asset(ID, &source, Some("valid.bin")).unwrap();
    let invalid = root
        .join(format!("assets/{ID}"))
        .join(OsStr::from_bytes(b"invalid-\xff.bin"));
    fs::write(&invalid, b"opaque").unwrap();
    failure(zk(root, &["asset", "list", ID]));
}

#[cfg(unix)]
#[test]
fn special_files_are_not_opened_copied_listed_or_removed() {
    use std::os::unix::net::UnixListener;
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    Archive::init(root).unwrap();
    note(root);
    let source = root.join("s");
    let _listener = UnixListener::bind(&source).unwrap();
    failure(zk(root, &["asset", "add", ID, "s"]));
    assert!(!root.join("assets").exists());
    let namespace = root.join(format!("assets/{ID}"));
    fs::create_dir_all(&namespace).unwrap();
    fs::rename(&source, namespace.join("s")).unwrap();
    failure(zk(root, &["asset", "list", ID]));
    failure(zk(root, &["asset", "remove", ID, "s"]));
    assert!(namespace.join("s").exists());
}
