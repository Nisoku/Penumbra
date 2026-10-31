use penumbra_markdown::links::{
    extract_inline_tags, extract_wikilinks, normalize_link_target, rewrite_wikilink_targets,
};
use penumbra_markdown::parser::parse_document;
use penumbra_markdown::render::markdown::render_markdown;


fn links_of(body: &str) -> Vec<String> {
    extract_wikilinks(&parse_document(body).unwrap())
}

fn tags_of(body: &str) -> Vec<String> {
    extract_inline_tags(&parse_document(body).unwrap())
}

#[test]
fn wikilinks_extract_deduped_in_order() {
    assert_eq!(
        links_of("See [[Alpha]] then [[Beta]] and [[Alpha]] again.\n"),
        vec!["Alpha".to_string(), "Beta".to_string()]
    );
}

#[test]
fn wikilinks_reduce_aliases_anchors_and_paths() {
    assert_eq!(normalize_link_target("Target|alias"), "Target");
    assert_eq!(normalize_link_target("Target#section"), "Target");
    assert_eq!(normalize_link_target("folder/sub/Target"), "Target");
    assert_eq!(
        links_of("[[Projects/Deep Note|the project]]\n"),
        vec!["Deep Note".to_string()]
    );
}

#[test]
fn wikilinks_inside_code_are_not_links() {
    assert_eq!(
        links_of("`[[Not a link]]` and\n\n```\n[[Also not]]\n```\n"),
        Vec::<String>::new()
    );
}

#[test]
fn unicode_body_text_stays_intact_around_links() {
    let doc = parse_document("Café résumé [[Ziel]] über #Étagère\n").unwrap();
    assert_eq!(
        doc.plain_text().split_whitespace().collect::<Vec<_>>(),
        vec!["Café", "résumé", "Ziel", "über", "Étagère"]
    );
}

#[test]
fn inline_tags_extract_with_unicode_and_nesting() {
    assert_eq!(
        tags_of("#alpha and #nested/tag plus #Ünïcodé, done.\n"),
        vec![
            "alpha".to_string(),
            "nested/tag".to_string(),
            "Ünïcodé".to_string()
        ]
    );
}

#[test]
fn hash_without_word_is_not_a_tag() {
    assert_eq!(tags_of("a # b #1 c #[[x]]\n"), Vec::<String>::new());
}

#[test]
fn rewrite_updates_bare_and_aliased_links_case_insensitively() {
    let body = "see [[old title]] and [[Old Title|alias]], keep `[[old title]]`, keep:\n\n    [[old title]]\n\n```\n[[old title]]\n```\n";
    let rewritten = rewrite_wikilink_targets(body, "Old Title", "New Title");
    assert!(rewritten.contains("[[New Title]]"));
    assert!(rewritten.contains("[[New Title|alias]]"));
    // The bare link is rewritten; the inline-code, indented, and fenced
    // copies stay untouched.
    assert_eq!(rewritten.matches("[[old title]]").count(), 3);
    assert_eq!(rewritten.matches("[[Old Title|").count(), 0);
}

const ROUNDTRIP_CORPUS: &[&str] = &[
    "# Heading",
    "## Heading 2\n\nBody text",
    "Paragraph with **bold**, *em*, ~~struck~~, `code`, and\nsoft break.\n\nAnd a line  \nhard break",
    "```rust\nfn main() {}\n```",
    "- one\n- two\n- three",
    "- [x] done\n- [ ] todo",
    "1. first\n2. second",
    "> quoted\n> \n> more",
    "before\n\n---\n\nafter",
    "| H1 | H2 |\n| --- | --- |\n| C1 | C2 |",
    "| a\\|b | **c** |\n| :--- | ---: |\n| x | y |",
    "<div>\nraw html\n</div>",
    "Press <kbd>Cmd</kbd> to continue",
    "A note[^1] with a reference.\n\n[^1]: The note text",
    "See [[other note]] and #tagged",
    "![alt text](https://example.com/img.png \"title\")",
    "A [link](https://example.com \"Title\") in prose",
];

#[test]
fn serializer_roundtrip_matches_source() {
    for (i, source) in ROUNDTRIP_CORPUS.iter().enumerate() {
        let doc = parse_document(source).expect("corpus item parses");
        let rendered = render_markdown(&doc);
        assert_eq!(
            rendered, *source,
            "roundtrip mismatch at corpus index {}",
            i
        );
    }
}

#[test]
fn serializer_roundtrip_is_stable() {
    for source in ROUNDTRIP_CORPUS {
        let doc = parse_document(source).expect("corpus item parses");
        let once = render_markdown(&doc);
        let twice = render_markdown(&parse_document(&once).expect("rendered re-parses"));
        assert_eq!(once, twice);
    }
}
