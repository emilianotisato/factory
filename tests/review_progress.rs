//! `collect::collect` in process, its progress sink fed by a stub `cursor-agent`. Alone in its binary: it sets the
//! process environment the reviewer, git and the data dir read.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::time::{Duration, Instant};

use factory::collect;
use factory::config::Review;
use factory::review::Progress;
use harness::Fixture;
use serde_json::json;

#[test]
fn review_passes_report_as_they_finish() {
    let fx = Fixture::new("review-progress");
    fx.git(&["checkout", "-q", "-b", "feat/review"]);
    for (f, n) in [("a", 300), ("b", 700)] {
        let body: String = (1..=n).map(|i| format!("{f} line {i}\n")).collect();
        fs::write(fx.path().join(f), body).unwrap();
        fx.git(&["add", f]);
    }
    fx.git(&["commit", "-q", "-m", "feat: the branch"]);
    // Pass 1 ends two seconds after pass 2, so finishing order is the reverse of pass order.
    fs::write(
        fx.stub_dir().join("cursor-agent.json"),
        json!({"sleep_secs": {"1": 2, "2": 0}}).to_string(),
    )
    .unwrap();
    for (k, v) in fx.env() {
        // SAFETY: the only test in this binary, and no thread of its own runs yet.
        unsafe { std::env::set_var(k, v) };
    }

    let cfg = Review {
        timeout_secs: 20,
        ..Review::default()
    };
    let mut heard = Vec::new();
    let mut sink = |p: Progress| {
        heard.push(p);
        Ok(())
    };
    let mut progress = Vec::new();
    let text = collect::collect(fx.path(), "main", &cfg, &mut progress, Some(&mut sink)).unwrap();
    assert!(text.contains("second opinion: reviewed, 2 of 2"), "{text}");

    println!("{heard:#?}");
    assert_eq!(heard.len(), 3, "{heard:?}");
    match &heard[0] {
        Progress::Started {
            passes: 2,
            dir: Some(dir),
        } => assert!(dir.is_dir(), "{}", dir.display()),
        other => panic!("first: {other:?}"),
    }
    let done: Vec<(usize, usize, u64)> = heard[1..]
        .iter()
        .map(|p| match p {
            Progress::PassDone { k, of, seconds } => (*k, *of, *seconds),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        done.iter().map(|d| (d.0, d.1)).collect::<Vec<_>>(),
        [(2, 2), (1, 2)],
        "{heard:?}"
    );
    assert!(done[1].2 >= 2, "{heard:?}");

    // A failing sink still waits out every reviewer, then the collect owes a rerun.
    let mut failing = |p: Progress| match p {
        Progress::Started { .. } => Ok(()),
        Progress::PassDone { .. } => Err(factory::Error::Msg("the sink broke".into())),
    };
    let started = Instant::now();
    let text =
        collect::collect(fx.path(), "main", &cfg, &mut progress, Some(&mut failing)).unwrap();
    assert!(started.elapsed() >= Duration::from_secs(2), "{text}");
    assert!(
        text.contains("second opinion: fallback owed — unreviewed: the sink broke"),
        "{text}"
    );
}
