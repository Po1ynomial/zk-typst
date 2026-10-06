use std::fs;
use std::process::Command;

use chrono::{Duration, NaiveDate};
use tempfile::tempdir;

fn zk() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zk"))
}

fn json_data(bytes: &[u8]) -> serde_json::Value {
    let envelope: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(envelope["schema_version"], 2);
    envelope
        .get("data")
        .expect("schema-2 data envelope")
        .clone()
}

fn initialize_descriptive(root: &std::path::Path) {
    let output = zk()
        .args(["init", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(
        root.join(zk::config::DEFAULT_TEMPLATE_PATH),
        zk::templates::DESCRIPTIVE_TEMPLATE,
    )
    .unwrap();
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
    assert!(!source.contains("#abstract"));
    assert!(!source.contains("@zk-field"));
    let node = zk()
        .current_dir(&root)
        .args(["query", "node", &id])
        .output()
        .unwrap();
    assert!(node.status.success());
    assert_eq!(json_data(&node.stdout)["metadata"], serde_json::json!({}));
    assert_eq!(
        fs::read_to_string(root.join("zk.toml")).unwrap(),
        "format = 3\n"
    );
}

#[test]
fn optionally_installs_archive_local_agent_skills() {
    let temporary = tempdir().unwrap();
    let plain = temporary.path().join("plain");
    let enabled = temporary.path().join("enabled");

    initialize_descriptive(&plain);
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
fn schema_two_fixtures_match_cli_results() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    initialize_descriptive(root);
    let empty = zk()
        .current_dir(root)
        .args(["graph", "--format", "json"])
        .output()
        .unwrap();
    assert!(empty.status.success());
    let actual: serde_json::Value = serde_json::from_slice(&empty.stdout).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema2/empty-graph.json")).unwrap();
    assert_eq!(actual, expected);
    fs::write(
        root.join("zk.toml"),
        include_str!("fixtures/schema2/zk.toml"),
    )
    .unwrap();
    fs::write(
        root.join(zk::config::DEFAULT_TEMPLATE_PATH),
        include_str!("fixtures/schema2/template.typ"),
    )
    .unwrap();
    fs::write(
        root.join("zettel/2603231410.typ"),
        include_str!("fixtures/schema2/note.typ"),
    )
    .unwrap();
    let node = zk()
        .current_dir(root)
        .args(["query", "node", "2603231410"])
        .output()
        .unwrap();
    assert!(node.status.success());
    let actual: serde_json::Value = serde_json::from_slice(&node.stdout).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema2/node.json")).unwrap();
    assert_eq!(actual, expected);
    let graph = zk()
        .current_dir(root)
        .args(["graph", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(json_data(&graph.stdout)["nodes"][0], expected["data"]);
    let clean = zk()
        .current_dir(root)
        .args(["check", "--format", "json"])
        .output()
        .unwrap();
    assert!(clean.status.success());
    assert_eq!(json_data(&clean.stdout), serde_json::json!([]));
    fs::write(
        root.join("zettel/2603231410.typ"),
        "= A note <2603231410>\n#tag(computed)\n",
    )
    .unwrap();
    let failed = zk()
        .current_dir(root)
        .args(["check", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(failed.status.code(), Some(1));
    assert!(failed.stderr.is_empty());
    let mut actual: serde_json::Value = serde_json::from_slice(&failed.stdout).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema2/diagnostics.json")).unwrap();
    assert!(actual["data"][0]["message"].is_string());
    // Wording is presentation, not a contract-fixture invariant.
    actual["data"][0]["message"] = expected["data"][0]["message"].clone();
    assert_eq!(actual, expected);
}

#[test]
fn explicit_metadata_is_user_owned_and_removed_fields_are_not_restored() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    initialize_descriptive(root);
    let manifest = fs::read_to_string(root.join("zk.toml")).unwrap();
    assert_eq!(manifest, "format = 3\n");
    let source = "= Source <2603231410>\n#keywords(computed)\n#tag(\"MixedCase\", \"duplicate\", \"duplicate\")\n#summary[Résumé]\n#group.coding\nBody-only needle.\n";
    fs::write(root.join("zettel/2603231410.typ"), source).unwrap();
    let custom = "= Title <new>\n// @zk-field \"tags\" kind=string-list\n#tag()\n\
              // @zk-field \"synopsis\" kind=markup\n#summary[]\n\
              // @zk-field \"topic\" kind=string\n#group.coding\n";
    fs::write(root.join(zk::config::DEFAULT_TEMPLATE_PATH), custom).unwrap();
    let node = zk()
        .current_dir(root)
        .args(["query", "node", "2603231410"])
        .output()
        .unwrap();
    let data = json_data(&node.stdout);
    assert_eq!(data["metadata"]["tags"]["kind"], "string-list");
    assert_eq!(
        data["metadata"]["tags"]["value"],
        serde_json::json!(["MixedCase", "duplicate", "duplicate"])
    );
    assert!(data["metadata"].get("keywords").is_none());
    for term in ["mixedcase", "RÉSUMÉ", "CODING"] {
        let output = zk()
            .current_dir(root)
            .args(["query", "search", term])
            .output()
            .unwrap();
        assert_eq!(json_data(&output.stdout).as_array().unwrap().len(), 1);
    }
    for term in ["tags", "needle", " MixedCase"] {
        let output = zk()
            .current_dir(root)
            .args(["query", "search", term])
            .output()
            .unwrap();
        assert_eq!(json_data(&output.stdout), serde_json::json!([]));
    }
    for empty in ["= Title <new>\n", "// ordinary comment\n= Title <new>\n"] {
        fs::write(root.join(zk::config::DEFAULT_TEMPLATE_PATH), empty).unwrap();
        let node = zk()
            .current_dir(root)
            .args(["query", "node", "2603231410"])
            .output()
            .unwrap();
        assert_eq!(json_data(&node.stdout)["metadata"], serde_json::json!({}));
        let check = zk()
            .current_dir(root)
            .args(["check", "--format", "json"])
            .output()
            .unwrap();
        assert!(check.status.success());
        assert_eq!(json_data(&check.stdout), serde_json::json!([]));
        let search = zk()
            .current_dir(root)
            .args(["query", "search", "MixedCase"])
            .output()
            .unwrap();
        assert_eq!(json_data(&search.stdout), serde_json::json!([]));
    }
    assert_eq!(
        fs::read_to_string(root.join("zettel/2603231410.typ")).unwrap(),
        source
    );
    assert_eq!(
        fs::read_to_string(root.join(zk::config::DEFAULT_TEMPLATE_PATH)).unwrap(),
        "// ordinary comment\n= Title <new>\n"
    );
}

#[test]
fn cli_statuses_and_global_archive_option_match_the_contract() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    initialize_descriptive(root);
    fs::write(
        root.join("zettel/2603231410.typ"),
        "= Source <2603231410>\n",
    )
    .unwrap();
    let version = zk().arg("--version").output().unwrap();
    assert_eq!(version.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(version.stdout).unwrap(),
        format!("zk {}\n", env!("CARGO_PKG_VERSION"))
    );
    for arguments in [vec!["graph"], vec!["query", "unknown"]] {
        let output = zk().args(arguments).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!output.stderr.is_empty());
    }
    for command in ["node", "links", "backlinks"] {
        let failed = zk()
            .current_dir(root)
            .args(["query", command, "9999999999"])
            .output()
            .unwrap();
        assert_eq!(failed.status.code(), Some(1));
        assert!(failed.stdout.is_empty());
    }
    let selected = zk()
        .args([
            "query",
            "node",
            "2603231410",
            "--archive",
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(selected.status.success());
    assert_eq!(json_data(&selected.stdout)["id"], "2603231410");
    for command in ["links", "backlinks"] {
        let output = zk()
            .current_dir(root)
            .args(["query", command, "2603231410"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(json_data(&output.stdout), serde_json::json!([]));
    }
}

#[test]
fn emits_a_disk_backed_json_graph() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize_descriptive(&root);
    write_zettel_with_metadata(
        &root,
        "2603231410",
        "Network _paths_",
        "Connects to @2603231411.",
        &["networks", "graph"],
        "thoughts",
        "Unicode before the range: café. Again @2603231411 and missing @9999999999.\n\
         The raw value `@8888888888` is not a link.\n\
         #let ignored = \"@7777777777\"\n\
         // @6666666666 is also not a link.",
    );
    write_zettel(&root, "2603231411", "Target", "Back to @2603231410.");
    write_zettel(
        &root,
        "2603231412",
        "Malformed",
        "Still links to @2603231411.",
    );
    let malformed_path = root.join("zettel/2603231412.typ");
    let malformed = fs::read_to_string(&malformed_path)
        .unwrap()
        .replace("<2603231412>", "<2603231499>")
        .replace("#keywords(\"test\")", "#keywords(\"valid\", computed)");
    fs::write(malformed_path, malformed).unwrap();
    write_zettel(&root, "2603231413", "Isolated", "No links.");
    fs::write(
        root.join("zettel/2603231414.typ"),
        "#import \"../lib/zettel.typ\": zettel, abstract, keywords, category\n\
         #show: zettel\n\n= Broken Typst <2603231414>\n\n#abstract[Unclosed block.",
    )
    .unwrap();
    fs::write(root.join("zettel/readme.typ"), "not a canonical Zettel").unwrap();

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
    let graph: serde_json::Value = json_data(&output.stdout);

    assert!(graph.get("schema_version").is_none());
    assert_eq!(graph["revision"], 1);
    assert_eq!(
        graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "2603231410",
            "2603231411",
            "2603231412",
            "2603231413",
            "2603231414"
        ]
    );
    assert_eq!(graph["nodes"][0]["title"]["text"], "Network paths");
    assert!(graph["nodes"][2]["metadata"]["keywords"].is_null());
    let links = graph["links"].as_array().unwrap();
    assert_eq!(links.len(), 4);
    assert_eq!(links[0]["source"], "2603231410");
    assert_eq!(links[0]["target"], "2603231411");
    assert_eq!(links[0]["resolution"], "resolved");
    assert_eq!(links[0]["spans"].as_array().unwrap().len(), 2);
    assert_eq!(links[1]["source"], "2603231410");
    assert_eq!(links[1]["target"], "9999999999");
    assert_eq!(links[1]["resolution"], "missing");
    assert_eq!(links[1]["spans"].as_array().unwrap().len(), 1);
    assert_eq!(links[2]["source"], "2603231411");
    assert_eq!(links[2]["target"], "2603231410");
    assert_eq!(links[2]["resolution"], "resolved");
    assert_eq!(links[3]["source"], "2603231412");
    assert_eq!(links[3]["target"], "2603231411");
    assert_eq!(links[3]["resolution"], "resolved");
    for link in links {
        let source = fs::read_to_string(
            root.join(format!("zettel/{}.typ", link["source"].as_str().unwrap())),
        )
        .unwrap();
        for span in link["spans"].as_array().unwrap() {
            let start = span["start"].as_u64().unwrap() as usize;
            let end = span["end"].as_u64().unwrap() as usize;
            assert_eq!(
                &source[start..end],
                format!("@{}", link["target"].as_str().unwrap())
            );
        }
    }
    let diagnostics = graph["diagnostics"].as_array().unwrap();
    for (path, code) in [
        ("zettel/2603231412.typ", "metadata.id_mismatch"),
        ("zettel/2603231412.typ", "metadata.invalid_shape"),
        ("zettel/2603231414.typ", "syntax.error"),
        ("zettel/readme.typ", "archive.filename"),
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic["path"] == path && diagnostic["code"] == code)
        );
    }
}

#[test]
fn relaxed_metadata_and_absent_fields_work_over_the_cli() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    initialize_descriptive(root);
    // Neither an imported implementation nor the bundled library is needed for extraction.
    fs::remove_file(root.join("lib/zettel.typ")).unwrap();
    let source = "#import \"styles.typ\": preamble, extra\n#show: preamble\n\
                  #keywords(\"one\")\nProse.\n#abstract[- Rich content]\n\
                  #set text(size: 10pt)\n= Flexible <2603231410>\n#category.coding\n";
    fs::write(root.join("zettel/2603231410.typ"), source).unwrap();
    fs::write(
        root.join("zettel/2603231411.typ"),
        "= Minimal <2603231411>\n",
    )
    .unwrap();
    fs::write(
        root.join("zettel/2603231412.typ"),
        "= Empty <2603231412>\n#abstract[]\n#keywords()\n",
    )
    .unwrap();
    let output = zk().current_dir(root).arg("check").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let output = zk()
        .current_dir(root)
        .args(["graph", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let graph: serde_json::Value = json_data(&output.stdout);
    assert!(graph["diagnostics"].as_array().unwrap().is_empty());
    assert_eq!(graph["nodes"][0]["title"]["text"], "Flexible");
    assert_eq!(graph["nodes"][0]["metadata"]["keywords"]["value"][0], "one");
    for field in ["abstract", "keywords", "category"] {
        assert!(graph["nodes"][1]["metadata"][field].is_null());
    }
    assert_eq!(
        graph["nodes"][2]["metadata"]["abstract"]["value"]["source"],
        ""
    );
    assert_eq!(
        graph["nodes"][2]["metadata"]["keywords"]["value"],
        serde_json::json!([])
    );
}

#[test]
fn new_reads_the_user_template_and_configured_source_forms() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    initialize_descriptive(root);
    let template = "#import \"../lib/styles.typ\": preamble\r\n#show: preamble\r\n\
                // @zk-field \"keywords\" kind=string-list\r\n#tags((\"custom\", \"template\"))\r\n\
                = Custom <new>\r\n\
                // @zk-field \"abstract\" kind=markup\r\n#summary[Unicode café.]\r\n\
                // @zk-field \"category\" kind=string\r\n#group.coding\r\n\r\n\
                // @zk-field malformed kind=markup\r\nID {{id}}.\r\n";
    let template_path = root.join(zk::config::DEFAULT_TEMPLATE_PATH);
    fs::write(&template_path, template).unwrap();
    let library = fs::read(root.join("lib/zettel.typ")).unwrap();
    let output = zk().current_dir(root).arg("new").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let relative = String::from_utf8(output.stdout).unwrap();
    let path = root.join(relative.trim());
    let id = path.file_stem().unwrap().to_str().unwrap();
    let rendered = fs::read_to_string(&path).unwrap();
    let expected = template
        .replace("<new>", &format!("<{id}>"))
        .replace("// @zk-field \"keywords\" kind=string-list", "")
        .replace("// @zk-field \"abstract\" kind=markup", "")
        .replace("// @zk-field \"category\" kind=string", "");
    assert_eq!(rendered, expected);
    assert_eq!(fs::read_to_string(&template_path).unwrap(), template);
    assert_eq!(fs::read(root.join("lib/zettel.typ")).unwrap(), library);
    let output = zk()
        .current_dir(root)
        .args(["query", "node", id])
        .output()
        .unwrap();
    assert!(output.status.success());
    let node: serde_json::Value = json_data(&output.stdout);
    assert_eq!(
        node["metadata"]["abstract"]["value"]["text"],
        "Unicode café."
    );
    assert_eq!(
        node["metadata"]["keywords"]["value"],
        serde_json::json!(["custom", "template"])
    );
    assert_eq!(node["metadata"]["category"]["value"], "coding");
    let range = &node["metadata"]["abstract"]["value"]["range"];
    assert_eq!(
        &rendered
            [range["start"].as_u64().unwrap() as usize..range["end"].as_u64().unwrap() as usize],
        "Unicode café."
    );
    let output = zk()
        .current_dir(root)
        .args(["query", "search", "TEMPLATE"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let nodes: serde_json::Value = json_data(&output.stdout);
    assert_eq!(nodes.as_array().unwrap().len(), 1);
}

#[test]
fn all_commands_reject_invalid_templates_without_replacing_them() {
    for template in [
        "= Title <new>\n#let broken = (",
        "= Missing label\n",
        "= One <new>\n= Two <other>\n",
        "= Title <new>\n// @zk-field \"summary\" kind=markup\n#summary(one)\n",
        "= Title <new>\n// @zk-field \"tags\" kind=string-list\n#tags(computed)\n",
        "= Title <new>\n// @zk-field \"summary\" kind=markup\n#summary[a]\n#summary[b]\n",
    ] {
        let temporary = tempdir().unwrap();
        let root = temporary.path();
        initialize_descriptive(root);
        let path = root.join(zk::config::DEFAULT_TEMPLATE_PATH);
        fs::write(&path, template).unwrap();
        for args in [
            vec!["new"],
            vec!["check"],
            vec!["check", "--format", "json"],
            vec!["graph", "--format", "json"],
            vec!["query", "search", ""],
            vec!["lsp"],
        ] {
            let output = zk().current_dir(root).args(args).output().unwrap();
            assert!(!output.status.success(), "accepted {template:?}");
            assert!(!output.stderr.is_empty());
        }
        assert_eq!(fs::read_dir(root.join("zettel")).unwrap().count(), 0);
        assert_eq!(fs::read_to_string(path).unwrap(), template);
    }
}

#[test]
fn missing_templates_are_created_with_only_the_core_even_during_inspection() {
    for args in [
        vec!["check"],
        vec!["graph", "--format", "json"],
        vec!["query", "search", ""],
        vec!["new"],
    ] {
        let temporary = tempdir().unwrap();
        let root = temporary.path();
        initialize_descriptive(root);
        let path = root.join(zk::config::DEFAULT_TEMPLATE_PATH);
        fs::remove_file(&path).unwrap();
        fs::remove_dir(path.parent().unwrap()).unwrap();
        let output = zk().current_dir(root).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read_to_string(path).unwrap(), zk::templates::ZETTEL);
        let graph = zk()
            .current_dir(root)
            .args(["graph", "--format", "json"])
            .output()
            .unwrap();
        for node in json_data(&graph.stdout)["nodes"].as_array().unwrap() {
            assert_eq!(node["metadata"], serde_json::json!({}));
        }
    }
}

#[test]
fn invalid_manifest_rules_are_errors_not_silent_defaults() {
    for config in [
        "[metadata.abstract]\nform = 'regex'\nname = 'summary'",
        "[metadata.abstract]\nform = 'content-call'\nname = 'abstract'\n[metadata.keywords]\nform = 'string-arguments-call'\nname = 'abstract'",
        "[metadata.abstract]\nform = 'content-call'\nname = 'module.summary'",
        "[metadata.abstract]\nname = 'summary'",
        "[new]\ntemplate = '../outside.tpl'",
        "[metadata]\nunknown = true",
    ] {
        let temporary = tempdir().unwrap();
        let root = temporary.path();
        initialize_descriptive(root);
        fs::write(root.join("zk.toml"), format!("format = 3\n{config}\n")).unwrap();
        let output = zk()
            .current_dir(root)
            .args(["graph", "--format", "json"])
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {config:?}");
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn init_preserves_existing_template_files() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    fs::create_dir(root.join("templates")).unwrap();
    let template = root.join(zk::config::DEFAULT_TEMPLATE_PATH);
    fs::write(&template, "user-owned").unwrap();
    let output = zk()
        .args(["init", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(template).unwrap(), "user-owned");
    assert!(!root.join("zk.toml").exists());
}

#[test]
fn isolated_zettel_has_no_diagnostic() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize_descriptive(&root);
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
    initialize_descriptive(&root);
    write_zettel(&root, "2603231410", "Dangling", "See @9999999999.");
    write_zettel(&root, "2603231411", "Isolated", "No links.");
    write_zettel(&root, "2603231412", "Malformed", "No links.");
    let malformed_path = root.join("zettel/2603231412.typ");
    let malformed = fs::read_to_string(&malformed_path)
        .unwrap()
        .replace("<2603231412>", "<2603231499>")
        .replace("#keywords(\"test\")", "#keywords(\"valid\", computed)");
    fs::write(malformed_path, malformed).unwrap();
    fs::write(root.join("zettel/bad-name.typ"), "invalid filename").unwrap();

    let output = zk()
        .current_dir(&root)
        .args(["check", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let diagnostics: serde_json::Value = json_data(&output.stdout);

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
    for code in ["metadata.id_mismatch", "metadata.invalid_shape"] {
        assert!(diagnostics.as_array().unwrap().iter().any(|diagnostic| {
            diagnostic["path"] == "zettel/2603231412.typ" && diagnostic["code"] == code
        }));
    }

    write_zettel(&root, "2603231410", "Repaired", "Now links to @2603231411.");
    write_zettel(&root, "2603231412", "Repaired metadata", "No links.");
    fs::remove_file(root.join("zettel/bad-name.typ")).unwrap();
    let repaired = zk().current_dir(&root).arg("check").output().unwrap();
    assert!(repaired.status.success());
    assert_eq!(
        String::from_utf8(repaired.stdout).unwrap(),
        "0 error(s), 0 warning(s)\n"
    );
}

#[test]
fn formatter_ordered_import_is_valid() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize_descriptive(&root);
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
    initialize_descriptive(&root);
    write_zettel(&root, "2603231410", "Source", "See @2603231411.");
    write_zettel(&root, "2603231411", "Target", "No outgoing links.");

    let node = zk()
        .current_dir(&root)
        .args(["query", "node", "2603231410"])
        .output()
        .unwrap();
    assert!(node.status.success());
    let node: serde_json::Value = json_data(&node.stdout);
    assert_eq!(node["title"]["text"], "Source");

    let links = zk()
        .current_dir(&root)
        .args(["query", "links", "2603231410"])
        .output()
        .unwrap();
    assert!(links.status.success());
    let links: serde_json::Value = json_data(&links.stdout);
    assert_eq!(links[0]["target"], "2603231411");

    let backlinks = zk()
        .current_dir(&root)
        .args(["query", "backlinks", "2603231411"])
        .output()
        .unwrap();
    assert!(backlinks.status.success());
    let backlinks: serde_json::Value = json_data(&backlinks.stdout);
    assert_eq!(backlinks[0]["source"], "2603231410");
}

#[test]
fn searches_all_metadata_fields_without_capping_or_reordering_results() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize_descriptive(&root);
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
    let nodes: serde_json::Value = json_data(&output.stdout);
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
    let id_nodes: serde_json::Value = json_data(&id_output.stdout);
    assert_eq!(id_nodes.as_array().unwrap().len(), 1);
    assert_eq!(id_nodes[0]["id"], "2603231412");

    let empty_output = zk()
        .current_dir(&root)
        .args(["query", "search", ""])
        .output()
        .unwrap();
    let all_nodes: serde_json::Value = json_data(&empty_output.stdout);
    assert_eq!(all_nodes.as_array().unwrap().len(), 105);
}

#[test]
fn removal_is_blocked_by_incoming_references() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("archive");
    initialize_descriptive(&root);
    write_zettel(
        &root,
        "2603231410",
        "Source",
        "See @2603231411 twice: @2603231411.",
    );
    write_zettel(&root, "2603231411", "Target", "No outgoing links.");
    write_zettel(
        &root,
        "2603231412",
        "Malformed",
        "Also links to @2603231411.",
    );
    let malformed_path = root.join("zettel/2603231412.typ");
    let malformed = fs::read_to_string(&malformed_path)
        .unwrap()
        .replace("<2603231412>", "<2603231499>")
        .replace("#keywords(\"test\")", "#keywords(\"valid\", computed)");
    fs::write(malformed_path, malformed).unwrap();

    let blocked = zk()
        .current_dir(&root)
        .args(["remove", "2603231411"])
        .output()
        .unwrap();
    assert_eq!(blocked.status.code(), Some(1));
    assert!(blocked.stdout.is_empty());
    assert!(root.join("zettel/2603231411.typ").is_file());
    let stderr = String::from_utf8_lossy(&blocked.stderr);
    assert!(stderr.contains("incoming references exist"));
    for id in ["2603231410", "2603231412"] {
        let relative = format!("zettel/{id}.typ");
        let source = fs::read_to_string(root.join(&relative)).unwrap();
        for (start, reference) in source.match_indices("@2603231411") {
            assert!(stderr.contains(&format!("{relative}:{start}..{}", start + reference.len())));
        }
    }
    let backlinks = zk()
        .current_dir(&root)
        .args(["query", "backlinks", "2603231411"])
        .output()
        .unwrap();
    assert!(backlinks.status.success());
    let backlinks: serde_json::Value = json_data(&backlinks.stdout);
    assert_eq!(backlinks[0]["source"], "2603231410");
    assert_eq!(backlinks[1]["source"], "2603231412");
    assert_eq!(backlinks.as_array().unwrap().len(), 2);

    let removed_source = zk()
        .current_dir(&root)
        .args(["remove", "2603231410"])
        .output()
        .unwrap();
    assert!(removed_source.status.success());
    assert!(!root.join("zettel/2603231410.typ").exists());
    assert_eq!(
        String::from_utf8(removed_source.stdout).unwrap(),
        "zettel/2603231410.typ\n"
    );

    let removed_malformed = zk()
        .current_dir(&root)
        .args(["remove", "2603231412"])
        .output()
        .unwrap();
    assert!(removed_malformed.status.success());
    assert!(!root.join("zettel/2603231412.typ").exists());

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
    initialize_descriptive(&local);
    initialize_descriptive(&selected);

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
    initialize_descriptive(&root);
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
    initialize_descriptive(&initialized);

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
