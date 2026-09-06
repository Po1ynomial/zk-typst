use std::fs;
use std::process::Command;

use tempfile::tempdir;

fn zk() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zk"))
}

fn initialize(root: &std::path::Path) {
    let output = zk()
        .args(["init", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_zettel(root: &std::path::Path, id: &str, title: &str, body: &str) {
    let source = format!(
        r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= {title} <{id}>

#abstract[Summary for {title}.]

#keywords("test")

#category.thoughts

{body}
"#
    );
    fs::write(root.join("zettel").join(format!("{id}.typ")), source).unwrap();
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
fn orphan_warning_does_not_fail_check() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize(&root);
    write_zettel(&root, "2603231410", "Orphan", "No links.");

    let output = zk().current_dir(&root).arg("check").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[graph.orphan]"));
    assert!(stdout.contains("0 error(s), 1 warning(s)"));
}

#[test]
fn check_reports_integrity_errors_and_orphan_warnings() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize(&root);
    write_zettel(&root, "2603231410", "Dangling", "See @9999999999.");
    write_zettel(&root, "2603231411", "Orphan", "No links.");
    fs::write(root.join("zettel/bad-name.typ"), "invalid filename").unwrap();

    let output = zk()
        .current_dir(&root)
        .args(["check", "--format", "json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(diagnostics.as_array().unwrap().iter().any(|diagnostic| {
        diagnostic["code"] == "reference.dangling" && diagnostic["path"] == "zettel/2603231410.typ"
    }));
    assert!(
        diagnostics
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "graph.orphan")
    );
    assert!(
        diagnostics
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "archive.filename")
    );
}

#[test]
fn queries_nodes_links_and_backlinks_as_json() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize(&root);
    write_zettel(&root, "2603231410", "Source", "See @2603231411.");
    write_zettel(&root, "2603231411", "Target", "No outgoing links.");

    let node = zk()
        .current_dir(&root)
        .args(["query", "node", "2603231410"])
        .output()
        .unwrap();
    assert!(node.status.success());
    let node: serde_json::Value = serde_json::from_slice(&node.stdout).unwrap();
    assert_eq!(node["title"]["text"], "Source");

    let links = zk()
        .current_dir(&root)
        .args(["query", "links", "2603231410"])
        .output()
        .unwrap();
    assert!(links.status.success());
    let links: serde_json::Value = serde_json::from_slice(&links.stdout).unwrap();
    assert_eq!(links[0]["target"], "2603231411");

    let backlinks = zk()
        .current_dir(&root)
        .args(["query", "backlinks", "2603231411"])
        .output()
        .unwrap();
    assert!(backlinks.status.success());
    let backlinks: serde_json::Value = serde_json::from_slice(&backlinks.stdout).unwrap();
    assert_eq!(backlinks[0]["source"], "2603231410");
}

#[test]
fn removal_is_blocked_by_incoming_references() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize(&root);
    write_zettel(&root, "2603231410", "Source", "See @2603231411.");
    write_zettel(&root, "2603231411", "Target", "No outgoing links.");

    let blocked = zk()
        .current_dir(&root)
        .args(["remove", "2603231411"])
        .output()
        .unwrap();
    assert!(!blocked.status.success());
    assert!(root.join("zettel/2603231411.typ").is_file());
    let stderr = String::from_utf8_lossy(&blocked.stderr);
    assert!(stderr.contains("incoming references exist"));
    assert!(stderr.contains("zettel/2603231410.typ:"));

    let removed_source = zk()
        .current_dir(&root)
        .args(["remove", "2603231410"])
        .output()
        .unwrap();
    assert!(removed_source.status.success());
    assert!(!root.join("zettel/2603231410.typ").exists());

    let removed_target = zk()
        .current_dir(&root)
        .args(["remove", "2603231411"])
        .output()
        .unwrap();
    assert!(removed_target.status.success());
    assert!(!root.join("zettel/2603231411.typ").exists());
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
