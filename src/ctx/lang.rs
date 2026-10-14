use std::cmp::Reverse;
use std::ops::Range;
use std::path::Path;
use std::sync::OnceLock;

use tree_sitter::{Language, Parser, Tree};
use tree_sitter_tags::{TagsConfiguration, TagsContext};

use super::vue;
use super::walk::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grammar {
    Php,
    Ts,
    Tsx,
    Go,
    Rust,
}

/// tree-sitter-tags keeps the lowest pattern's tag per name, so patterns put in front of upstream win.
const PHP_EXTRA: &str = r"
(method_declaration name: (name) @name) @definition.method
(enum_declaration name: (name) @name) @definition.class
";

/// TypeScript's `tags.scm` leaves functions, classes and methods to JavaScript's, which this crate does not ship.
const TS_EXTRA: &str = r"
(function_declaration name: (identifier) @name) @definition.function
(generator_function_declaration name: (identifier) @name) @definition.function
(class_declaration name: (type_identifier) @name) @definition.class
(method_definition name: (_) @name) @definition.method
(lexical_declaration (variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression) (generator_function)])) @definition.function
(variable_declaration (variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression) (generator_function)])) @definition.function
(type_alias_declaration name: (type_identifier) @name) @definition.type
(enum_declaration name: (identifier) @name) @definition.class
";

const GO_EXTRA: &str = r"
(type_spec name: (type_identifier) @name type: [(struct_type) (interface_type)]) @definition.class
(type_alias name: (type_identifier) @name) @definition.type
";

/// An `impl` block is a definition so the outline can nest its methods under it.
const RUST_EXTRA: &str = r"
(type_item name: (type_identifier) @name) @definition.type
(mod_item body: (declaration_list (function_item name: (identifier) @name) @definition.function))
(trait_item body: (declaration_list (function_signature_item name: (identifier) @name) @definition.method))
(const_item name: (identifier) @name) @definition.constant
(static_item name: (identifier) @name) @definition.constant
(impl_item type: (_) @name) @definition.impl
";

impl Grammar {
    const ALL: usize = 5;

    /// Only `.ts` is plain TypeScript: JSX may appear in any other script extension.
    pub fn of(path: &Path, lang: Lang) -> Grammar {
        match lang {
            Lang::Php => Grammar::Php,
            Lang::Go => Grammar::Go,
            Lang::Rust => Grammar::Rust,
            Lang::Vue => Grammar::Ts,
            Lang::Ts => match path.extension().and_then(|e| e.to_str()) {
                Some("ts") => Grammar::Ts,
                _ => Grammar::Tsx,
            },
        }
    }

    pub fn language(self) -> Language {
        match self {
            Grammar::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Grammar::Ts => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Grammar::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Grammar::Go => tree_sitter_go::LANGUAGE.into(),
            Grammar::Rust => tree_sitter_rust::LANGUAGE.into(),
        }
    }

    /// No locals query: locals only hide references, and only definitions are read here.
    pub fn tags_config(self) -> &'static TagsConfiguration {
        static CONFIGS: [OnceLock<TagsConfiguration>; Grammar::ALL] =
            [const { OnceLock::new() }; Grammar::ALL];
        CONFIGS[self as usize].get_or_init(|| {
            let (extra, upstream) = match self {
                Grammar::Php => (PHP_EXTRA, tree_sitter_php::TAGS_QUERY),
                Grammar::Ts | Grammar::Tsx => (TS_EXTRA, tree_sitter_typescript::TAGS_QUERY),
                Grammar::Go => (GO_EXTRA, tree_sitter_go::TAGS_QUERY),
                Grammar::Rust => (RUST_EXTRA, tree_sitter_rust::TAGS_QUERY),
            };
            TagsConfiguration::new(self.language(), &format!("{extra}\n{upstream}"), "")
                .expect("the built-in tags queries compile")
        })
    }

    pub fn is_comment_or_string(self, kind: &str) -> bool {
        match self {
            Grammar::Php => matches!(
                kind,
                "comment" | "string" | "encapsed_string" | "heredoc" | "nowdoc"
            ),
            Grammar::Ts | Grammar::Tsx => matches!(kind, "comment" | "string" | "template_string"),
            Grammar::Go => matches!(
                kind,
                "comment" | "interpreted_string_literal" | "raw_string_literal"
            ),
            Grammar::Rust => matches!(
                kind,
                "line_comment" | "block_comment" | "string_literal" | "raw_string_literal"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub range: Range<usize>,
    pub name: Range<usize>,
    /// tree-sitter-tags cuts it at 180 bytes.
    pub line: Range<usize>,
    pub kind: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Unit<'a> {
    pub grammar: Grammar,
    pub offset: usize,
    pub text: &'a str,
}

impl<'a> Unit<'a> {
    pub fn all(path: &Path, lang: Lang, text: &'a str) -> Vec<Unit<'a>> {
        if lang == Lang::Vue {
            return vue::script_blocks(text)
                .into_iter()
                .map(|(offset, script)| Unit {
                    grammar: Grammar::Ts,
                    offset,
                    text: script,
                })
                .collect();
        }
        vec![Unit {
            grammar: Grammar::of(path, lang),
            offset: 0,
            text,
        }]
    }

    pub fn parse(&self) -> Option<Tree> {
        let mut parser = Parser::new();
        parser.set_language(&self.grammar.language()).ok()?;
        parser.parse(self.text, None)
    }

    pub fn definitions(&self) -> Option<(Vec<Definition>, bool)> {
        let config = self.grammar.tags_config();
        let mut cx = TagsContext::new();
        let (tags, has_error) = cx.generate_tags(config, self.text.as_bytes(), None).ok()?;
        let shift = |r: Range<usize>| r.start + self.offset..r.end + self.offset;
        let mut defs = tags
            .filter_map(Result::ok)
            .filter(|t| t.is_definition)
            .map(|t| Definition {
                range: shift(t.range),
                name: shift(t.name_range),
                line: shift(t.line_range),
                kind: kind(config.syntax_type_name(t.syntax_type_id)),
            })
            .collect::<Vec<_>>();
        defs.sort_by_key(|d| (d.range.start, Reverse(d.range.end), d.name.start));
        Some((defs, has_error))
    }

    pub fn in_comment_or_string(&self, tree: &Tree, at: Range<usize>) -> bool {
        let Some(start) = at.start.checked_sub(self.offset) else {
            return false;
        };
        let end = at.end - self.offset;
        if end > self.text.len() {
            return false;
        }
        let mut node = tree.root_node().descendant_for_byte_range(start, end);
        while let Some(n) = node {
            if n.kind() == "template_substitution" {
                return false;
            }
            if self.grammar.is_comment_or_string(n.kind()) {
                return true;
            }
            node = n.parent();
        }
        false
    }
}

pub fn definitions(units: &[Unit]) -> Option<(Vec<Definition>, bool)> {
    let mut all = Vec::new();
    let mut broken = false;
    for unit in units {
        let (defs, error) = unit.definitions()?;
        all.extend(defs);
        broken |= error;
    }
    all.sort_by_key(|d| (d.range.start, Reverse(d.range.end), d.name.start));
    Some((all, broken))
}

pub struct Parsed<'a>(Vec<(Unit<'a>, Tree)>);

impl<'a> Parsed<'a> {
    pub fn of(units: &[Unit<'a>]) -> Option<Parsed<'a>> {
        units
            .iter()
            .map(|u| Some((*u, u.parse()?)))
            .collect::<Option<Vec<_>>>()
            .map(Parsed)
    }

    pub fn has_error(&self) -> bool {
        self.0.iter().any(|(_, t)| t.root_node().has_error())
    }

    pub fn in_comment_or_string(&self, at: Range<usize>) -> bool {
        self.0
            .iter()
            .any(|(u, t)| u.in_comment_or_string(t, at.clone()))
    }
}

pub fn kind(syntax: &'static str) -> &'static str {
    match syntax {
        "class" | "interface" | "trait" | "enum" | "struct" => "class",
        "function" => "fn",
        other => other,
    }
}

pub fn kind_matches(want: Option<&str>, kind: &str) -> bool {
    want.is_none_or(|w| w == kind || (w == "fn" && kind == "method"))
}

pub fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

pub fn line_of(starts: &[usize], at: usize) -> usize {
    starts.partition_point(|&s| s <= at).saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(rel: &str) -> String {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixture/ctx")
            .join(rel);
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn each_grammar_parses_its_fixture_and_tags_a_definition() {
        for (rel, lang, root) in [
            ("php/UserService.php", Lang::Php, "program"),
            ("refs/caller01.ts", Lang::Ts, "program"),
            ("go/client.go", Lang::Go, "source_file"),
            ("rs/config.rs", Lang::Rust, "source_file"),
            ("vue/UserCard.vue", Lang::Vue, "program"),
        ] {
            let text = fixture(rel);
            let unit = Unit::all(Path::new(rel), lang, &text)[0];
            let tree = unit.parse().unwrap();
            assert_eq!(tree.root_node().kind(), root, "{rel}");
            assert!(!tree.root_node().has_error(), "{rel}");
            let (defs, broken) = unit.definitions().unwrap();
            assert!(!broken && !defs.is_empty(), "{rel}: {defs:?}");
        }
        for g in [
            Grammar::Php,
            Grammar::Ts,
            Grammar::Tsx,
            Grammar::Go,
            Grammar::Rust,
        ] {
            assert!(g.tags_config().syntax_type_name(0).len() > 1, "{g:?}");
        }
    }

    #[test]
    fn kinds_map_to_the_kind_vocabulary() {
        let text = fixture("php/UserService.php");
        let unit = Unit::all(Path::new("a.php"), Lang::Php, &text)[0];
        let (defs, _) = unit.definitions().unwrap();
        let named = |n: &str| {
            defs.iter()
                .find(|d| &text[d.name.clone()] == n)
                .map(|d| d.kind)
        };
        assert_eq!(named("UserService"), Some("class"));
        assert_eq!(named("handle"), Some("method"));
        assert!(kind_matches(Some("fn"), "method"));
        assert!(!kind_matches(Some("method"), "fn"));
        assert!(!kind_matches(Some("class"), "method"));
        assert!(kind_matches(None, "module"));
    }

    #[test]
    fn comments_and_strings_are_found_in_every_grammar() {
        for (path, lang, src) in [
            (
                "a.php",
                Lang::Php,
                "<?php\n// x\n$a = 'x';\n$b = \"x\";\nx();\n",
            ),
            (
                "a.ts",
                Lang::Ts,
                "// x\nconst a = 'x';\nconst b = `x ${x()}`;\nx();\n",
            ),
            (
                "a.go",
                Lang::Go,
                "package a\n// x\nvar a = \"x\" + `x`\nfunc b() { x() }\n",
            ),
            (
                "a.rs",
                Lang::Rust,
                "// x\nconst A: &str = \"x\"; /* x */\nfn b() { x() }\n",
            ),
        ] {
            let unit = Unit::all(Path::new(path), lang, src)[0];
            let tree = unit.parse().unwrap();
            let lines = line_starts(src);
            let marks: Vec<bool> = src
                .match_indices('x')
                .map(|(i, _)| unit.in_comment_or_string(&tree, i..i + 1))
                .collect();
            let code = marks.iter().filter(|m| !**m).count();
            assert!(code >= 1 && marks.iter().any(|m| *m), "{path}: {marks:?}");
            let last = src.rfind("x(").unwrap();
            assert!(!unit.in_comment_or_string(&tree, last..last + 1), "{path}");
            assert_eq!(line_of(&lines, last), src[..last].matches('\n').count());
        }
        let src = "const b = `x ${x()}`;\n";
        let unit = Unit::all(Path::new("a.ts"), Lang::Ts, src)[0];
        let tree = unit.parse().unwrap();
        assert!(unit.in_comment_or_string(&tree, 11..12));
        assert!(!unit.in_comment_or_string(&tree, 15..16));
    }
}
