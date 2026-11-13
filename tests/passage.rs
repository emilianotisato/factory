#[path = "common/passage.rs"]
mod passage;

use passage::assert_unique_passage;
use std::panic::catch_unwind;

#[test]
fn a_passage_must_occur_exactly_once() {
    assert_unique_passage("one", "a rule, then another", "a rule");
    for (label, text) in [("zero", "nothing here"), ("two", "a rule and a rule")] {
        let Err(err) = catch_unwind(|| assert_unique_passage(label, text, "a rule")) else {
            panic!("passed on {label}");
        };
        let msg = err.downcast_ref::<String>().expect("a formatted message");
        assert!(msg.starts_with(&format!("{label}: ")), "{msg}");
    }
}
