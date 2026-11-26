use akidev_core::model::Release;

#[test]
fn file_scoped_sources_have_stable_distinct_selection_ids() {
    let r = |file_idx| Release {
        title: "Show S01 1080p / Show.S01E01.1080p.mkv".into(),
        info_hash: "a".repeat(40),
        file_idx,
        magnet: "magnet:?xt=urn:btih:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
        size_bytes: None,
        seeders: Some(5),
        source: "torrentio".into(),
        sources: vec!["torrentio".into()],
    };
    assert_ne!(r(Some(0)).selection_id(), r(Some(1)).selection_id());
    assert_eq!(r(None).selection_id().len(), 40);
    assert_eq!(
        r(Some(0)).selection_id(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:0"
    );

    assert_eq!(
        Release::parse_selection_id(&r(Some(12)).selection_id()),
        Some(("a".repeat(40), Some(12)))
    );
    assert_eq!(
        Release::parse_selection_id(&r(None).selection_id()),
        Some(("a".repeat(40), None))
    );
    assert_eq!(
        Release::parse_selection_id("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA:0"),
        Some(("a".repeat(40), Some(0)))
    );
    for bad in [
        "",
        "short",
        "gggggggggggggggggggggggggggggggggggggggg",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:-1",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:+1",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1x",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:184467440737095516160",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1:2",
    ] {
        assert_eq!(Release::parse_selection_id(bad), None, "{bad}");
    }
}
