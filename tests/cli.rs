use std::fs;
use std::process::Command;

use tempfile::tempdir;

fn zk() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zk"))
}

#[test]
fn initializes_an_archive_and_creates_a_zettel_from_a_nested_directory() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");

    let init = zk()
        .args(["init", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(root.join("zk.toml").is_file());
    assert!(root.join("lib/zettel.typ").is_file());
    assert!(root.join("zettel").is_dir());

    let nested = root.join("assets");
    fs::create_dir(&nested).unwrap();
    let new = zk().current_dir(&nested).arg("new").output().unwrap();
    assert!(
        new.status.success(),
        "new failed: {}",
        String::from_utf8_lossy(&new.stderr)
    );

    let relative = String::from_utf8(new.stdout).unwrap();
    let relative = relative.trim();
    assert!(relative.starts_with("zettel/"));
    assert!(relative.ends_with(".typ"));

    let path = root.join(relative);
    let id = path.file_stem().unwrap().to_str().unwrap().to_owned();
    assert_eq!(id.len(), 10);
    assert!(id.bytes().all(|byte| byte.is_ascii_digit()));

    let source = fs::read_to_string(&path).unwrap();
    assert!(source.contains(&format!("= Untitled <{id}>")));
    assert!(source.contains("#abstract[]"));
    assert!(source.contains("#keywords()"));
    assert!(source.contains("#category.thoughts"));
}

#[test]
fn new_fails_outside_an_archive() {
    let temporary = tempdir().unwrap();

    let output = zk()
        .current_dir(temporary.path())
        .arg("new")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no archive found"));
}
