//! The Vue shim (PRD §14 FR-12): Vue has no `tags.scm`, so the first
//! `<script>` block is cut out and parsed with the TypeScript grammar, and
//! every range is moved back by the block's offset in the file. The
//! `<template>` is never parsed.

/// The first `<script>` block's inner text and its byte offset in `src`;
/// none when the file has no complete block. `setup`, `lang="ts"` or no
/// `lang` all parse as TypeScript, a superset for this purpose. A `>`
/// inside a quoted attribute (`generic="T extends Map<K, V>"`) does not
/// close the tag.
pub fn script_block(src: &str) -> Option<(usize, &str)> {
    let mut from = 0;
    let open = loop {
        let at = from + src[from..].find("<script")?;
        let next = src[at + "<script".len()..].chars().next()?;
        if next == '>' || next.is_whitespace() {
            break at;
        }
        from = at + "<script".len();
    };
    let mut quote = None;
    let mut start = None;
    for (i, c) in src[open..].char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => {
                start = Some(open + i + 1);
                break;
            }
            (None, _) => {}
        }
    }
    let start = start?;
    let end = start + src[start..].find("</script>")?;
    Some((start, &src[start..end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_script_block_comes_out_with_its_offset() {
        let src = "<template><p/></template>\n<script setup lang=\"ts\">\nconst a = 1\n</script>\n";
        let (offset, text) = script_block(src).unwrap();
        assert_eq!(text, "\nconst a = 1\n");
        assert_eq!(&src[offset..offset + text.len()], text);

        let src = "<script setup lang=\"ts\" generic=\"T extends Map<string, number>\">x</script>";
        assert_eq!(script_block(src).map(|(_, t)| t), Some("x"));

        let src = "<scripts>no</scripts><script>yes</script><script setup>no</script>";
        assert_eq!(script_block(src).map(|(_, t)| t), Some("yes"));

        assert_eq!(script_block("<template><p>hi</p></template>\n"), None);
        assert_eq!(script_block("<script>unclosed"), None);
    }
}
