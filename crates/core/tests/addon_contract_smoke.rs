use akidev_core::model::Release;

#[test]
fn stremio_file_scoped_rows_have_exact_distinct_selection_ids() {
    let make = |file_idx| Release {
        title: "Show S01 1080p / Show.S01E01.1080p.mkv".into(),
        info_hash: "a".repeat(40),
        file_idx,
        magnet: String::new(),
        size_bytes: None,
        seeders: None,
        source: "torrentio".into(),
        sources: vec!["torrentio".into()],
    };
    assert_eq!(make(None).selection_id().len(), 40);
    assert_ne!(make(Some(0)).selection_id(), make(Some(1)).selection_id());
}
