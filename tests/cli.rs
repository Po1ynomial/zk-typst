use std::fs;
use std::process::Command;

use chrono::{Duration, NaiveDate};
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
    write_zettel_with_metadata(
        root,
        id,
        title,
        &format!("Summary for {title}."),
        &["test"],
        "thoughts",
        body,
    );
}

fn write_zettel_with_metadata(
    root: &std::path::Path,
    id: &str,
    title: &str,
    abstract_text: &str,
    keywords: &[&str],
    category: &str,
    body: &str,
) {
    let keywords = keywords
        .iter()
        .map(|keyword| format!("\"{keyword}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= {title} <{id}>

#abstract[{abstract_text}]

#keywords({keywords})

#category.{category}

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
fn optionally_installs_archive_local_agent_skills() {
    let temporary = tempdir().unwrap();
    let plain = temporary.path().join("plain");
    let enabled = temporary.path().join("enabled");

    initialize(&plain);
    assert!(!plain.join(".agents").exists());

    let output = zk()
        .args([
            "init",
            "--agent-skills",
            enabled.to_str().expect("temporary path is UTF-8"),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let skill = fs::read_to_string(enabled.join(".agents/skills/zettelkasten/SKILL.md")).unwrap();
    assert!(skill.starts_with("---\nname: zettelkasten\n"));
    assert!(skill.contains("zk query search"));
    assert!(skill.contains("Broad or empty searches can produce large JSON output."));
    assert!(skill.contains("Connection matters more than collecting"));
    assert!(skill.contains("The prose around a link should state why the target is relevant."));
}

#[test]
fn skill_installation_warns_without_overwriting_or_failing_init() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    let skill = root.join(".agents/skills/zettelkasten/SKILL.md");
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    fs::write(&skill, "user-owned\n").unwrap();

    let output = zk()
        .args([
            "init",
            "--agent-skills",
            root.to_str().expect("temporary path is UTF-8"),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(root.join("zk.toml").is_file());
    assert_eq!(fs::read_to_string(skill).unwrap(), "user-owned\n");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning"));
    assert!(stderr.contains("agent skill `zettelkasten` already exists"));
}

#[test]
fn skill_installation_io_failure_does_not_fail_archive_creation() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    fs::create_dir(&root).unwrap();
    fs::write(root.join(".agents"), "path conflict\n").unwrap();

    let output = zk()
        .args([
            "init",
            "--agent-skills",
            root.to_str().expect("temporary path is UTF-8"),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(root.join("zk.toml").is_file());
    assert_eq!(
        fs::read_to_string(root.join(".agents")).unwrap(),
        "path conflict\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning"));
    assert!(stderr.contains("cannot install agent skill `zettelkasten`"));
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
fn isolated_zettel_has_no_diagnostic() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize(&root);
    write_zettel(&root, "2603231410", "Isolated", "No links.");

    let output = zk().current_dir(&root).arg("check").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("[graph.orphan]"));
    assert!(stdout.contains("0 error(s), 0 warning(s)"));
}

#[test]
fn check_reports_integrity_errors() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize(&root);
    write_zettel(&root, "2603231410", "Dangling", "See @9999999999.");
    write_zettel(&root, "2603231411", "Isolated", "No links.");
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
            .all(|diagnostic| diagnostic["code"] != "graph.orphan")
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
fn formatter_ordered_import_is_valid() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize(&root);
    write_zettel(&root, "2603231410", "Formatted", "No links.");
    let path = root.join("zettel/2603231410.typ");
    let source = fs::read_to_string(&path).unwrap().replacen(
        "zettel, abstract, keywords, category",
        "abstract, category, keywords, zettel",
        1,
    );
    fs::write(path, source).unwrap();

    let output = zk().current_dir(&root).arg("check").output().unwrap();

    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("[metadata.import]"));
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
fn searches_all_metadata_fields_without_capping_or_reordering_results() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize(&root);
    write_zettel_with_metadata(
        &root,
        "2603231410",
        "Path efficiency",
        "First summary.",
        &["networks"],
        "thoughts",
        "No links.",
    );
    write_zettel_with_metadata(
        &root,
        "2603231411",
        "Second",
        "A PATH through the archive.",
        &["retrieval"],
        "thoughts",
        "No links.",
    );
    write_zettel_with_metadata(
        &root,
        "2603231412",
        "Third",
        "Another summary.",
        &["pathfinding"],
        "thoughts",
        "No links.",
    );
    write_zettel_with_metadata(
        &root,
        "2603231413",
        "Fourth",
        "Last summary.",
        &["retrieval"],
        "pathways",
        "No links.",
    );
    let start = NaiveDate::from_ymd_opt(2026, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    for offset in 0..101 {
        let id = (start + Duration::minutes(offset))
            .format("%y%m%d%H%M")
            .to_string();
        write_zettel_with_metadata(
            &root,
            &id,
            "Bulk",
            "Bulk summary.",
            &["bulk"],
            "bulk",
            "No links.",
        );
    }

    let output = zk()
        .current_dir(&root)
        .args(["query", "search", "PaTh"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let nodes: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ids = nodes
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        ["2603231410", "2603231411", "2603231412", "2603231413"]
    );
    assert_eq!(nodes[0]["title"]["text"], "Path efficiency");

    let id_output = zk()
        .current_dir(&root)
        .args(["query", "search", "1412"])
        .output()
        .unwrap();
    let id_nodes: serde_json::Value = serde_json::from_slice(&id_output.stdout).unwrap();
    assert_eq!(id_nodes.as_array().unwrap().len(), 1);
    assert_eq!(id_nodes[0]["id"], "2603231412");

    let empty_output = zk()
        .current_dir(&root)
        .args(["query", "search", ""])
        .output()
        .unwrap();
    let all_nodes: serde_json::Value = serde_json::from_slice(&empty_output.stdout).unwrap();
    assert_eq!(all_nodes.as_array().unwrap().len(), 105);
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

#[test]
fn explicit_archive_overrides_local_discovery() {
    let temporary = tempdir().unwrap();
    let local = temporary.path().join("local");
    let selected = temporary.path().join("selected");
    initialize(&local);
    initialize(&selected);

    let output = zk()
        .current_dir(&local)
        .args(["--archive", selected.to_str().unwrap(), "new"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "new failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_dir(local.join("zettel")).unwrap().count(), 0);
    assert_eq!(fs::read_dir(selected.join("zettel")).unwrap().count(), 1);
}

#[test]
fn explicit_relative_archive_supports_existing_archive_commands() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    let outside = temporary.path().join("outside");
    initialize(&root);
    fs::create_dir(&outside).unwrap();
    write_zettel(&root, "2603231410", "Selected", "No links.");

    let archive = "../archive";
    for arguments in [
        vec!["--archive", archive, "check"],
        vec!["--archive", archive, "query", "node", "2603231410"],
        vec!["--archive", archive, "graph", "--format", "json"],
    ] {
        let output = zk().current_dir(&outside).args(arguments).output().unwrap();
        assert!(
            output.status.success(),
            "command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let removed = zk()
        .current_dir(&outside)
        .args(["--archive", archive, "remove", "2603231410"])
        .output()
        .unwrap();
    assert!(
        removed.status.success(),
        "remove failed: {}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(!root.join("zettel/2603231410.typ").exists());
}

#[test]
fn explicit_archive_rejects_invalid_roots_and_init() {
    let temporary = tempdir().unwrap();
    let missing = temporary.path().join("missing");
    let initialized = temporary.path().join("initialized");
    let target = temporary.path().join("target");
    initialize(&initialized);

    let invalid = zk()
        .current_dir(temporary.path())
        .args(["--archive", missing.to_str().unwrap(), "check"])
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("no archive found"));

    let init = zk()
        .args([
            "--archive",
            initialized.to_str().unwrap(),
            "init",
            target.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!init.status.success());
    assert!(String::from_utf8_lossy(&init.stderr).contains("cannot be used with `init`"));
    assert!(!target.exists());
}
