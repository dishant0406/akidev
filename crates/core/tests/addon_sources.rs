use akidev_core::model::Release;
use akidev_core::releases::rank_releases;

#[test]
fn source_dedupe_retains_file_scoped_rows() {
    let rows = [0usize, 1].map(|index| Release {
        title: format!(
            "Show.S01.1080p.WEB-DL.x264 / Show/S01/Show.S01E01.{quality}.WEB-DL.x264.mkv",
            quality = if index == 0 { "1080p" } else { "720p" }
        ),
        info_hash: "a".repeat(40),
        file_idx: Some(index),
        magnet: String::new(),
        size_bytes: None,
        seeders: Some(10),
        source: "torrentio".into(),
        sources: vec!["torrentio".into()],
    });
    assert_eq!(rank_releases(&rows, 1, "Show", &[], Some(1)).len(), 2);
}
