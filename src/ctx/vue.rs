//! Vue has no `tags.scm`: each `<script>` block, whatever its `lang`, parses as TypeScript (a superset here) at its offset.

pub fn script_blocks(src: &str) -> Vec<(usize, &str)> {
    let mut blocks = Vec::new();
    let mut from = 0;
    while let Some((start, end)) = next_block(src, from) {
        blocks.push((start, &src[start..end]));
        from = end + "</script>".len();
    }
    blocks
}

fn next_block(src: &str, mut from: usize) -> Option<(usize, usize)> {
    let open = loop {
        let at = from + src.get(from..)?.find("<script")?;
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
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(src: &str) -> Vec<&str> {
        script_blocks(src).into_iter().map(|(_, t)| t).collect()
    }

    #[test]
    fn every_script_block_comes_out_with_its_offset() {
        let src = "<template><p/></template>\n<script setup lang=\"ts\">\nconst a = 1\n</script>\n";
        let blocks = script_blocks(src);
        assert_eq!(blocks.len(), 1);
        let (offset, text) = blocks[0];
        assert_eq!(text, "\nconst a = 1\n");
        assert_eq!(&src[offset..offset + text.len()], text);

        let src = "<script setup lang=\"ts\" generic=\"T extends Map<string, number>\">x</script>";
        assert_eq!(texts(src), ["x"]);

        let src = "<scripts>no</scripts><script>one</script><template/><script setup>two</script>";
        assert_eq!(texts(src), ["one", "two"]);
        for (offset, text) in script_blocks(src) {
            assert_eq!(&src[offset..offset + text.len()], text);
        }

        assert!(texts("<template><p>hi</p></template>\n").is_empty());
        assert!(texts("<script>unclosed").is_empty());
        assert_eq!(texts("<script>a</script><script>unclosed"), ["a"]);
    }
}
