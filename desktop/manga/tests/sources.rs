//! Every definition under `sources/` must read its recorded pages as its fixtures say. This is what
//! stops a change to a theme from quietly breaking the sites that extend it.

use akidev_manga::{check, theme::Themes};
use std::path::Path;

#[tokio::test]
async fn every_definition_reads_its_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../manga-sources");
    let mut themes = Themes::default();
    for entry in std::fs::read_dir(root.join("themes")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        themes.insert(&name, serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap());
    }
    let mut checked = 0;
    let definitions = ["sources", "examples"].into_iter().flat_map(|dir| std::fs::read_dir(root.join(dir)).unwrap().map(|entry| entry.unwrap().path()));
    for path in definitions {
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

/// The committed `index.json` must be what `index` would write now: a changed definition with a stale index
/// would be refused by every app (its hash would not match).
#[test]
fn the_index_is_up_to_date() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../manga-sources");
    let committed: akidev_manga::repo::Index = serde_json::from_str(&std::fs::read_to_string(root.join("index.json")).unwrap()).unwrap();
    let fresh = akidev_manga::repo::build_index(&root, committed.key.clone()).unwrap();
    assert_eq!(committed, fresh, "run: cargo run -p akidev-manga --bin akidev-manga-cli -- index ../manga-sources");
}
