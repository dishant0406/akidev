//! Every definition under `sources/` must read its recorded pages as its fixtures say. This is what
//! stops a change to a theme from quietly breaking the sites that extend it.

use akidev_manga::{check, theme::Themes};
use std::path::Path;

#[tokio::test]
async fn every_definition_reads_its_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("sources");
    let mut themes = Themes::default();
    for entry in std::fs::read_dir(root.join("themes")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        themes.insert(&name, serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap());
    }
    let mut checked = 0;
    for entry in std::fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let id = path.file_stem().unwrap().to_str().unwrap().to_string();
        let def = themes.resolve(serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()).unwrap_or_else(|error| panic!("{id}: {error:#}"));
        let outcomes = check::run(def, &root.join("fixtures").join(&id), false).await.unwrap_or_else(|error| panic!("{id}: {error:#}"));
        assert!(!outcomes.is_empty(), "{id} has no fixture calls");
        for outcome in outcomes {
            assert!(outcome.error.is_none(), "{id}: {}: {}", outcome.name, outcome.error.unwrap());
        }
        checked += 1;
    }
    assert!(checked > 0, "no definitions found");
}
