//! A skill-text assertion that a deleted rule cannot pass through a sibling line sharing its words.
#![allow(dead_code)]

/// Panics unless `needle` occurs exactly once in `text`; `label` names the file or section `text` came from.
pub fn assert_unique_passage(label: &str, text: &str, needle: &str) {
    let count = text.matches(needle).count();
    assert!(
        count == 1,
        "{label}: {needle:?} occurs {count} times, want exactly once"
    );
}
