use std::error::Error;
use std::fs;

use zk::archive::Archive;
use zk::model::LinkResolution;
use zk::provider::{Provider, UpdateOutcome};

fn zettel(id: &str, title: &str, body: &str) -> String {
    format!(
        r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= {title} <{id}>

#abstract[Overlay inspection note.]

#keywords("inspection")

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

fn main() -> Result<(), Box<dyn Error>> {
    let root = std::env::args().nth(1).expect("archive path argument");
    let archive = Archive::discover(&root)?;
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
    eprintln!(
        "prepared disk update: generation {}",
        pending_disk.generation()
    );

    let opened = provider.open_buffer(
        &source_path,
        1,
        zettel("2603231410", "Overlay source", "Link @2603231412."),
    )?;
    eprintln!("open disk-backed buffer: {opened:?}");
    assert!(matches!(opened, UpdateOutcome::Applied { revision: 2, .. }));
    assert_eq!(title(&provider, "2603231410"), "Overlay source");
    assert_eq!(
        provider.links_from("2603231410")[0].resolution,
        LinkResolution::Missing
    );

    let current_generation = provider.node("2603231410").expect("source node").generation;
    let rejected_disk = provider.apply_prepared(pending_disk);
    eprintln!("apply superseded disk update: {rejected_disk:?}");
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
    eprintln!("open unsaved buffer: {opened_unsaved:?}");
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
    eprintln!("repeat document version: {stale_version:?}");
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
    eprintln!("change open buffer: {changed:?}");
    assert!(matches!(
        changed,
        UpdateOutcome::Applied { revision: 4, .. }
    ));
    assert_eq!(provider.links_to("2603231411").len(), 2);
    assert_eq!(provider.links_from("2603231410")[0].spans.len(), 2);

    let saved = provider.save_buffer(&source_path)?;
    eprintln!("save open buffer: {saved:?}");
    assert!(matches!(
        saved,
        UpdateOutcome::OverlayRetained { revision: 4, .. }
    ));
    fs::write(
        &source_path,
        zettel("2603231410", "Saved disk", "Link @2603231412."),
    )?;
    let ignored_disk = provider.refresh_disk(&source_path)?;
    eprintln!("refresh disk under overlay: {ignored_disk:?}");
    assert_eq!(ignored_disk, UpdateOutcome::IgnoredOpenOverlay);
    assert_eq!(title(&provider, "2603231410"), "Changed overlay");

    let closed_source = provider.close_buffer(&source_path)?;
    eprintln!("close saved buffer: {closed_source:?}");
    assert!(matches!(
        closed_source,
        UpdateOutcome::Applied { revision: 5, .. }
    ));
    assert_eq!(title(&provider, "2603231410"), "Saved disk");

    let closed_unsaved = provider.close_buffer(&session_path)?;
    eprintln!("close unsaved buffer: {closed_unsaved:?}");
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
    eprintln!("restore closed disk node: {restored:?}");
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

    eprintln!("overlay lifecycle assertions passed through revision 7");
    serde_json::to_writer_pretty(std::io::stdout().lock(), &provider.snapshot())?;
    println!();
    Ok(())
}
