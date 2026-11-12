//! `review::run_passes` with a cancel check, over stub reviewers that sleep. Alone in its binary: it sets the
//! process environment the reviewer reads.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::time::{Duration, Instant};

use factory::config::Review;
use factory::review::{self, Ended, Pass};
use factory::run::pid_alive;
use harness::Fixture;
use serde_json::json;

#[test]
fn a_cancel_check_kills_every_running_pass() {
    let fx = Fixture::new("review-stop");
    fs::write(
        fx.stub_dir().join("cursor-agent.json"),
        json!({"sleep_secs": 60, "spawn_child": true}).to_string(),
    )
    .unwrap();
    for (k, v) in fx.env() {
        // SAFETY: the only test in this binary, and no thread of its own runs yet.
        unsafe { std::env::set_var(k, v) };
    }
    let cfg = Review {
        timeout_secs: 120,
        ..Review::default()
    };
    let passes: Vec<Pass> = ["a", "b", "c"]
        .iter()
        .map(|f| Pass {
            files: vec![f.to_string()],
            lines: 10,
            over: false,
            part: None,
        })
        .collect();
    let dir = fx.path().join("reviews");
    fs::create_dir(&dir).unwrap();
    let pid_file = |k: usize, kind: &str| fx.stub_dir().join(format!("cursor-agent-{k}.{kind}"));
    let children: Vec<_> = (1..=3).map(|k| pid_file(k, "child.pid")).collect();
    let started = Instant::now();
    let stop =
        move || children.iter().all(|p| p.exists()) || started.elapsed() >= Duration::from_secs(20);

    let ran = review::run_passes(
        &cfg,
        fx.path(),
        &dir,
        &passes,
        |n, _| {
            Ok(format!(
                "Review {}",
                dir.join(format!("review-{n}.diff")).display()
            ))
        },
        &mut Vec::new(),
        None,
        Some(&stop),
    )
    .unwrap();

    let took = started.elapsed();
    assert!(took < Duration::from_secs(10), "{took:?}");
    assert_eq!(ran.len(), 3);
    for (i, r) in ran.iter().enumerate() {
        let o = r.outcome.as_ref().unwrap();
        assert_eq!(o.ended, Ended::Stopped, "pass {}", i + 1);
        assert_eq!(
            review::pass_text(r, cfg.timeout_secs),
            ("unreviewed (stopped)\n".to_string(), false)
        );
        for kind in ["pid", "child.pid"] {
            let pid: u32 = fs::read_to_string(pid_file(i + 1, kind))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let gone = Instant::now();
            while pid_alive(pid) && gone.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(!pid_alive(pid), "pass {} {kind} {pid} still runs", i + 1);
        }
    }
}
