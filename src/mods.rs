//! Listed by hand; `tests/cockpit_install.rs` fails on a file under `mods/cockpit/` this list misses.

pub const COCKPIT_FILES: &[(&str, &str)] = &[
    (
        ".claude-plugin/plugin.json",
        include_str!("../mods/cockpit/.claude-plugin/plugin.json"),
    ),
    (
        "hooks/hooks.json",
        include_str!("../mods/cockpit/hooks/hooks.json"),
    ),
    (
        "hooks/register.tsx",
        include_str!("../mods/cockpit/hooks/register.tsx"),
    ),
    (
        "tests/cockpit.test.tsx",
        include_str!("../mods/cockpit/tests/cockpit.test.tsx"),
    ),
    (
        "types/index.d.ts",
        include_str!("../mods/cockpit/types/index.d.ts"),
    ),
];
