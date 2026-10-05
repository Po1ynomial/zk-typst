use std::error::Error;
use std::fs;

use tempfile::tempdir;
use zk::archive::Archive;
use zk::model::LinkResolution;
use zk::provider::{Provider, UpdateOutcome};

fn zettel(id: &str, title: &str, body: &str) -> String {
    format!(
        r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= {title} <{id}>

#abstract[Overlay lifecycle test.]

#keywords("test")

#category.thoughts

{body}
"#
    )
}

fn title<'a>(provider: &'a Provider, id: &str) -> &'a str {
    &provider
        .node(id)
        .expect("node exists")
        .title
        .as_ref()
        .expect("title exists")
        .text
}

#[test]
fn overlay_lifecycle_preserves_coherent_revisions_and_snapshot() -> Result<(), Box<dyn Error>> {
    let temporary = tempdir()?;
    let archive = Archive::init(temporary.path())?;
    fs::write(
        archive.root().join("zettel/2603231410.typ"),
        zettel("2603231410", "Disk source", "Link @2603231411."),
    )?;
    fs::write(
        archive.root().join("zettel/2603231411.typ"),
        zettel("2603231411", "Disk target", "No outgoing links."),
    )?;
    let source_path = archive.root().join("zettel/2603231410.typ");
    let session_path = archive.root().join("zettel/2603231412.typ");
    let mut provider = Provider::load(&archive)?;
    assert_eq!(provider.revision(), 1);
    assert_eq!(title(&provider, "2603231410"), "Disk source");

    fs::write(
        &source_path,
        zettel("2603231410", "Pending disk", "Link @2603231411."),
    )?;
    let pending_disk = provider
        .prepare_disk_update(&source_path)?
        .expect("closed file accepts disk preparation");

    let opened = provider.open_buffer(
        &source_path,
        1,
        zettel("2603231410", "Overlay source", "Link @2603231412."),
    )?;
    assert!(matches!(opened, UpdateOutcome::Applied { revision: 2, .. }));
    assert_eq!(title(&provider, "2603231410"), "Overlay source");
    assert_eq!(
        provider.links_from("2603231410")[0].resolution,
        LinkResolution::Missing
    );

    let current_generation = provider.node("2603231410").expect("source node").generation;
    let rejected_disk = provider.apply_prepared(pending_disk);
    assert_eq!(
        rejected_disk,
        UpdateOutcome::StaleGeneration {
            current: current_generation,
        }
    );
    assert_eq!(provider.revision(), 2);

    let opened_unsaved = provider.open_buffer(
        &session_path,
        1,
        zettel("2603231412", "Unsaved target", "Link @2603231411."),
    )?;
    assert!(matches!(
        opened_unsaved,
        UpdateOutcome::Applied { revision: 3, .. }
    ));
    assert_eq!(
        provider.links_from("2603231410")[0].resolution,
        LinkResolution::Resolved
    );

    let stale_version = provider.change_buffer(
        &source_path,
        1,
        &zettel("2603231410", "Stale change", "Link @2603231411."),
    )?;
    assert_eq!(stale_version, UpdateOutcome::StaleVersion { current: 1 });
    assert_eq!(provider.revision(), 3);

    let changed = provider.change_buffer(
        &source_path,
        2,
        &zettel(
            "2603231410",
            "Changed overlay",
            "Twice @2603231411 and @2603231411.",
        ),
    )?;
    assert!(matches!(
        changed,
        UpdateOutcome::Applied { revision: 4, .. }
    ));
    assert_eq!(provider.links_to("2603231411").len(), 2);
    assert_eq!(provider.links_from("2603231410")[0].spans.len(), 2);

    let saved = provider.save_buffer(&source_path)?;
    assert!(matches!(
        saved,
        UpdateOutcome::OverlayRetained { revision: 4, .. }
    ));
    fs::write(
        &source_path,
        zettel("2603231410", "Saved disk", "Link @2603231412."),
    )?;
    let ignored_disk = provider.refresh_disk(&source_path)?;
    assert_eq!(ignored_disk, UpdateOutcome::IgnoredOpenOverlay);
    assert_eq!(title(&provider, "2603231410"), "Changed overlay");

    let closed_source = provider.close_buffer(&source_path)?;
    assert!(matches!(
        closed_source,
        UpdateOutcome::Applied { revision: 5, .. }
    ));
    assert_eq!(title(&provider, "2603231410"), "Saved disk");

    let closed_unsaved = provider.close_buffer(&session_path)?;
    assert!(matches!(
        closed_unsaved,
        UpdateOutcome::Applied { revision: 6, .. }
    ));
    assert!(provider.node("2603231412").is_none());
    assert_eq!(
        provider.links_from("2603231410")[0].resolution,
        LinkResolution::Missing
    );

    fs::write(
        &session_path,
        zettel("2603231412", "Restored disk target", "Link @2603231411."),
    )?;
    let restored = provider.refresh_disk(&session_path)?;
    assert!(matches!(
        restored,
        UpdateOutcome::Applied { revision: 7, .. }
    ));
    assert_eq!(title(&provider, "2603231412"), "Restored disk target");
    assert_eq!(
        provider.links_from("2603231410")[0].resolution,
        LinkResolution::Resolved
    );
    assert!(provider.diagnostics().is_empty());

    let snapshot = provider.snapshot();
    assert_eq!(snapshot.revision, 7);
    assert_eq!(
        snapshot
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        ["2603231410", "2603231411", "2603231412"]
    );
    assert_eq!(snapshot.links.len(), 2);
    assert_eq!(snapshot.links[0].source, "2603231410");
    assert_eq!(snapshot.links[0].target, "2603231412");
    assert_eq!(snapshot.links[1].source, "2603231412");
    assert_eq!(snapshot.links[1].target, "2603231411");
    assert!(
        snapshot
            .links
            .iter()
            .all(|link| link.resolution == LinkResolution::Resolved)
    );
    assert!(snapshot.diagnostics.is_empty());
    Ok(())
}
