//! Not a test of behaviour: times the engine's reads on a big library, so a change can be shown to help or not.
//! Run: cargo test -p akidev-manga-ffi --profile ios --test perf_bench -- --nocapture --ignored
use akidev_manga::model::{Chapter, Status};
use akidev_manga::store::{Progress, Store};
use akimanga::Engine;
use std::time::Instant;

fn time<T>(name: &str, runs: u32, mut work: impl FnMut() -> T) {
    let mut samples = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        std::hint::black_box(work());
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "BENCH {name:<34} median {:>8.2} ms   worst {:>8.2} ms",
        samples[samples.len() / 2],
        samples[samples.len() - 1]
    );
}

#[test]
#[ignore]
fn engine_reads_on_a_big_library() {
    let dir = std::env::temp_dir().join(format!("akimanga-perf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    {
        let store = Store::open(&dir.join("manga.db")).unwrap();
        for s in 0..400 {
            let id = format!("series-{s}");
            store
                .upsert_series(
                    "src",
                    &id,
                    &format!("Series number {s}"),
                    Some("https://x/c.jpg"),
                    None,
                    Status::Ongoing,
                )
                .unwrap();
            store.add_to_library("src", &id, 1_000 + s).unwrap();
            let chapters: Vec<Chapter> = (0..80)
                .rev()
                .map(|n| Chapter {
                    id: format!("c{n}"),
                    number: Some(n as f64),
                    title: None,
                    date_ms: Some(1_700_000_000_000 + n * 86_400_000),
                    scanlator: None,
                })
                .collect();
            store.sync_chapters("src", &id, &chapters).unwrap();
            for n in 0..(s % 60) {
                store
                    .set_progress(
                        "src",
                        &id,
                        &format!("c{n}"),
                        Progress {
                            page: 3,
                            offset: 0.2,
                            finished: true,
                        },
                        10_000 + n,
                    )
                    .unwrap();
            }
        }
    }
    time("Engine::new (open + migrate)", 5, || {
        Engine::new(dir.to_string_lossy().to_string()).unwrap()
    });
    let engine = Engine::new(dir.to_string_lossy().to_string()).unwrap();
    time("library()", 15, || engine.library().unwrap());
    time("updates()", 15, || engine.updates().unwrap());
    time("history(24)", 15, || engine.history(24).unwrap());
    time("categories()", 15, || engine.categories().unwrap());
    time("cached_chapters (one series)", 15, || {
        engine
            .cached_chapters("src".into(), "series-7".into())
            .unwrap()
    });
    time("resume (one series)", 15, || {
        engine.resume("src".into(), "series-7".into()).unwrap()
    });
}
