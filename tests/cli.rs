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
fn emits_a_disk_backed_json_graph() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    let init = zk()
        .args(["init", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(init.status.success());

    let source = r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Source <2603231410>

#abstract[Summary]

#keywords("graph")

#category.thoughts

See @2603231411 twice: @2603231411.
"#;
    fs::write(root.join("zettel/2603231410.typ"), source).unwrap();

    let output = zk()
        .current_dir(&root)
        .args(["graph", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "graph failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let graph: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(graph["schema_version"], 1);
    assert_eq!(graph["nodes"][0]["title"]["text"], "Source");
    assert_eq!(graph["links"][0]["target"], "2603231411");
    assert_eq!(graph["links"][0]["resolution"], "missing");
    assert_eq!(graph["links"][0]["spans"].as_array().unwrap().len(), 2);
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
