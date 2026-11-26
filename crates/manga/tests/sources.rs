//! The engine's own theme and example definitions must read their recorded pages as their fixtures say.
//! This is what stops a change to a theme from quietly breaking the definitions that extend it.
//!
//! These are synthetic fixtures (the `madara` theme and the `madara-example` / `madara-protected`
//! definitions that exercise it), kept under `desktop/manga/testdata`. The catalogue of real source
//! definitions lives in its own repository, `github.com/dishant0406/manga-sources`, and is checked there.

use akidev_manga::{check, theme::Themes};
use std::path::Path;

#[tokio::test]
async fn every_definition_reads_its_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
    let mut themes = Themes::default();
    for entry in std::fs::read_dir(root.join("themes")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        themes.insert(
            &name,
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap(),
        );
    }
    let mut checked = 0;
    for entry in std::fs::read_dir(root.join("examples")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let id = path.file_stem().unwrap().to_str().unwrap().to_string();
        let def = themes
            .resolve(serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap())
            .unwrap_or_else(|error| panic!("{id}: {error:#}"));
        let outcomes = check::run(def, &root.join("fixtures").join(&id), false)
            .await
            .unwrap_or_else(|error| panic!("{id}: {error:#}"));
        assert!(!outcomes.is_empty(), "{id} has no fixture calls");
        for outcome in outcomes {
            assert!(
                outcome.error.is_none(),
                "{id}: {}: {}",
                outcome.name,
                outcome.error.unwrap()
            );
        }
        checked += 1;
    }
    assert!(checked > 0, "no definitions found");
}
