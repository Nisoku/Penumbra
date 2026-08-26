use penumbra_markdown::ast::BlockKind;
use penumbra_markdown::ast::Document;
use penumbra_markdown::ast::Inline;
use penumbra_markdown::ast::Inline::*;
use penumbra_markdown::frontmatter::{parse as parse_fm, serialize as serialize_fm, Frontmatter};
use penumbra_markdown::parser::markdown_to_html;
use penumbra_markdown::parser::markdown_to_plain;
use penumbra_markdown::parser::parse_document;
use penumbra_markdown::render::html::render_html;
use penumbra_markdown::render::text::render_plain;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

// Parser tests

fn assert_para(doc: &Document, idx: usize, expected: &[Inline]) {
    match &doc.blocks[idx].kind {
        BlockKind::Paragraph(children) => {
            assert_eq!(children.len(), expected.len(), "para {} len", idx);
            for (i, (a, b)) in children.iter().zip(expected.iter()).enumerate() {
                assert_eq!(a, b, "mismatch inline {} para {}", i, idx);
            }
        }
        other => panic!("expected Paragraph at {}, got {:?}", idx, other),
    }
}

#[test]
fn parser_empty() {
    let doc = parse_document("").unwrap();
    assert!(doc.is_empty());
}

#[test]
fn parser_plain() {
    let doc = parse_document("hello world").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    assert_para(&doc, 0, &[Text("hello world".into())]);
}

#[test]
fn parser_heading() {
    let doc = parse_document("# Hello").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::Heading { level, children } => {
            assert_eq!(*level, 1);
            assert_eq!(children, &[Text("Hello".into())]);
        }
        other => panic!("expected Heading, got {:?}", other),
    }
}

#[test]
fn parser_heading_levels() {
    for (input, want) in [("## A", 2), ("### A", 3), ("#### A", 4)] {
        let doc = parse_document(input).unwrap();
        match &doc.blocks[0].kind {
            BlockKind::Heading { level, .. } => assert_eq!(*level, want),
            other => panic!("got {:?}", other),
        }
    }
}

#[test]
fn parser_embed() {
    let doc = parse_document("hello [[id-123]] world").unwrap();
    assert_para(
        &doc,
        0,
        &[
            Text("hello ".into()),
            NoteEmbed {
                note_ref: "id-123".into(),
            },
            Text(" world".into()),
        ],
    );
}

#[test]
fn parser_embed_prefix() {
    let doc = parse_document("[[embed]] text").unwrap();
    assert_para(
        &doc,
        0,
        &[
            NoteEmbed {
                note_ref: "embed".into(),
            },
            Text(" text".into()),
        ],
    );
}

#[test]
fn parser_embed_only() {
    let doc = parse_document("[[only]]").unwrap();
    assert_para(
        &doc,
        0,
        &[NoteEmbed {
            note_ref: "only".into(),
        }],
    );
}

#[test]
fn parser_multi_embed() {
    let doc = parse_document("[[a]] and [[b]]").unwrap();
    assert_para(
        &doc,
        0,
        &[
            NoteEmbed {
                note_ref: "a".into(),
            },
            Text(" and ".into()),
            NoteEmbed {
                note_ref: "b".into(),
            },
        ],
    );
}

#[test]
fn parser_tag() {
    let doc = parse_document("hello #world here").unwrap();
    assert_para(
        &doc,
        0,
        &[
            Text("hello ".into()),
            TagRef {
                name: "world".into(),
            },
            Text(" here".into()),
        ],
    );
}

#[test]
fn parser_tag_hyphen() {
    let doc = parse_document("check #urgent-task now").unwrap();
    assert_para(
        &doc,
        0,
        &[
            Text("check ".into()),
            TagRef {
                name: "urgent-task".into(),
            },
            Text(" now".into()),
        ],
    );
}

#[test]
fn parser_tag_punct_end() {
    let doc = parse_document("end #tag.").unwrap();
    assert_para(
        &doc,
        0,
        &[
            Text("end ".into()),
            TagRef { name: "tag".into() },
            Text(".".into()),
        ],
    );
}

#[test]
fn parser_double_hash() {
    let doc = parse_document("## heading").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::Heading { level, .. } => assert_eq!(*level, 2),
        other => panic!("expected Heading, got {:?}", other),
    }
}

#[test]
fn parser_fenced_code() {
    let doc = parse_document("```rust\nfn main() {}\n```").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::CodeBlock { language, text } => {
            assert_eq!(language.as_deref(), Some("rust"));
            assert!(text.contains("fn main()"));
        }
        other => panic!("expected CodeBlock, got {:?}", other),
    }
}

#[test]
fn parser_indented_code() {
    let doc = parse_document("    code line").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::CodeBlock { language, text } => {
            assert!(language.is_none());
            assert_eq!(text.trim(), "code line");
        }
        other => panic!("expected CodeBlock, got {:?}", other),
    }
}

#[test]
fn parser_strong_em() {
    let doc = parse_document("**bold** and *italic*").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::Paragraph(children) => {
            assert_eq!(children.len(), 3);
            match &children[0] {
                Strong(c) => assert_eq!(c, &[Text("bold".into())]),
                other => panic!("expected Strong, got {:?}", other),
            }
            assert_eq!(children[1], Text(" and ".into()));
            match &children[2] {
                Emphasis(c) => assert_eq!(c, &[Text("italic".into())]),
                other => panic!("expected Emphasis, got {:?}", other),
            }
        }
        other => panic!("expected Paragraph, got {:?}", other),
    }
}

#[test]
fn parser_strikethrough() {
    let doc = parse_document("~~struck~~").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::Paragraph(children) => match &children[0] {
            Strikethrough(c) => assert_eq!(c, &[Text("struck".into())]),
            other => panic!("expected Strikethrough, got {:?}", other),
        },
        other => panic!("expected Paragraph, got {:?}", other),
    }
}

#[test]
fn parser_link() {
    let doc = parse_document("[text](http://example.com)").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::Paragraph(children) => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                Link {
                    url,
                    title,
                    children,
                } => {
                    assert_eq!(url, "http://example.com");
                    assert_eq!(title, "");
                    assert_eq!(children, &[Text("text".into())]);
                }
                other => panic!("expected Link, got {:?}", other),
            }
        }
        other => panic!("expected Paragraph, got {:?}", other),
    }
}

#[test]
fn parser_link_title() {
    let doc = parse_document("[text](http://ex.com \"Title\")").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::Paragraph(children) => match &children[0] {
            Link { title, .. } => assert!(title.contains("Title")),
            other => panic!("expected Link, got {:?}", other),
        },
        other => panic!("expected Paragraph, got {:?}", other),
    }
}

#[test]
fn parser_image() {
    let doc = parse_document("![alt](http://ex.com/img.png)").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::Paragraph(children) => match &children[0] {
            Image { url, alt, .. } => {
                assert_eq!(url, "http://ex.com/img.png");
                assert_eq!(alt, "alt");
            }
            other => panic!("expected Image, got {:?}", other),
        },
        other => panic!("expected Paragraph, got {:?}", other),
    }
}

#[test]
fn parser_ulist() {
    let doc = parse_document("- item one\n- item two").unwrap();
    assert!(!doc.blocks.is_empty());
    match &doc.blocks[0].kind {
        BlockKind::List { ordered, items, .. } => {
            assert!(!ordered);
            assert_eq!(items.len(), 2);
        }
        other => panic!("expected List, got {:?}", other),
    }
}

#[test]
fn parser_olist() {
    let doc = parse_document("1. first\n2. second").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::List { ordered, items, .. } => {
            assert!(ordered);
            assert_eq!(items.len(), 2);
        }
        other => panic!("expected List, got {:?}", other),
    }
}

#[test]
fn parser_quote() {
    let doc = parse_document("> quoted text").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::Quote(children) => assert!(!children.is_empty()),
        other => panic!("expected Quote, got {:?}", other),
    }
}

#[test]
fn parser_hr() {
    let doc = parse_document("---\n").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    assert_eq!(doc.blocks[0].kind, BlockKind::ThematicBreak);
}

#[test]
fn parser_two_paras() {
    let doc = parse_document("para one\n\npara two").unwrap();
    assert_eq!(doc.blocks.len(), 2);
    assert_para(&doc, 0, &[Text("para one".into())]);
    assert_para(&doc, 1, &[Text("para two".into())]);
}

#[test]
fn parser_inline_code() {
    let doc = parse_document("use `code` here").unwrap();
    assert_para(
        &doc,
        0,
        &[
            Text("use ".into()),
            Code("code".into()),
            Text(" here".into()),
        ],
    );
}

#[test]
fn parser_plain_text() {
    let doc = parse_document("# Hello\n\nThis is **bold** and #tag\n\n[[embed-ref]]").unwrap();
    let plain = doc.plain_text();
    assert!(plain.contains("Hello"));
    assert!(plain.contains("bold"));
    assert!(plain.contains("tag"));
    assert!(plain.contains("embed-ref"));
}

#[test]
fn parser_mixed() {
    let doc = parse_document("# A\n\nB **C** D\n\n- list").unwrap();
    assert_eq!(doc.blocks.len(), 3);
    assert!(matches!(
        doc.blocks[0].kind,
        BlockKind::Heading { level: 1, .. }
    ));
    assert!(matches!(doc.blocks[1].kind, BlockKind::Paragraph(_)));
    assert!(matches!(doc.blocks[2].kind, BlockKind::List { .. }));
}

#[test]
fn parser_table() {
    let doc = parse_document("| H1 | H2 |\n|---|---|\n| C1 | C2 |").unwrap();
    assert!(!doc.blocks.is_empty());
    match &doc.blocks[0].kind {
        BlockKind::Table(t) => {
            assert!(!t.rows.is_empty());
            assert_eq!(t.rows[0].len(), 2);
        }
        other => panic!("expected Table, got {:?}", other),
    }
}

#[test]
fn parser_footnote() {
    let doc = parse_document("Text[^1]\n\n[^1]: Note").unwrap();
    assert!(!doc.blocks.is_empty());
}

#[test]
fn parser_task_list() {
    let doc = parse_document("- [x] done\n- [ ] todo").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::List { items, .. } => assert_eq!(items.len(), 2),
        other => panic!("expected List, got {:?}", other),
    }
}

#[test]
fn parser_html_rt() {
    let html = markdown_to_html("hello **world**").unwrap();
    assert!(html.contains("<strong>world</strong>"));
    assert!(html.contains("hello"));
}

#[test]
fn parser_plain_rt() {
    let plain = markdown_to_plain("# Title\n\nBody text").unwrap();
    assert!(plain.contains("Title"));
    assert!(plain.contains("Body text"));
}

#[test]
fn parser_embed_inside_strong() {
    let doc = parse_document("**[[only]]**").unwrap();
    match &doc.blocks[0].kind {
        BlockKind::Paragraph(children) => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                Strong(c) => {
                    assert_eq!(c.len(), 1);
                    match &c[0] {
                        NoteEmbed { note_ref } => assert_eq!(note_ref, "only"),
                        other => panic!("expected NoteEmbed, got {:?}", other),
                    }
                }
                other => panic!("expected Strong, got {:?}", other),
            }
        }
        other => panic!("expected Paragraph, got {:?}", other),
    }
}

// HTML render tests

#[test]
fn render_para() {
    let doc = parse_document("hello world").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<p>"));
    assert!(html.contains("hello world"));
}

#[test]
fn render_h1() {
    let doc = parse_document("# Title").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<h1>Title</h1>"));
}

#[test]
fn render_embed() {
    let doc = parse_document("before [[some-id]] after").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<pe-embed data-ref=\"some-id\">"));
}

#[test]
fn render_tag() {
    let doc = parse_document("text #mytag").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<pe-tag data-name=\"mytag\">"));
}

#[test]
fn render_code_block() {
    let doc = parse_document("```rust\nfn main() {}\n```").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("class=\"language-rust\""));
}

#[test]
fn render_ul() {
    let doc = parse_document("- a\n- b").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<ul>"));
    assert!(html.contains("<li>"));
}

#[test]
fn render_ol() {
    let doc = parse_document("1. first\n2. second").unwrap();
    let html = render_html(&doc);
    assert!(html.starts_with("<ol"));
}

#[test]
fn render_quote() {
    let doc = parse_document("> quote").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<blockquote>"));
}

#[test]
fn render_hr() {
    let doc = parse_document("---").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<hr>"));
}

#[test]
fn render_link() {
    let doc = parse_document("[text](http://ex.com)").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<a href=\"http://ex.com\""));
}

#[test]
fn render_image() {
    let doc = parse_document("![alt](http://ex.com/img.png)").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<img src=\"http://ex.com/img.png\" alt=\"alt\""));
}

#[test]
fn render_strong_em() {
    let doc = parse_document("**bold** and *italic*").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<em>italic</em>"));
}

#[test]
fn render_code() {
    let doc = parse_document("use `f()`").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<code>f()</code>"));
}

#[test]
fn sanitize_script() {
    let doc = parse_document("<script>alert(1)</script>hello").unwrap();
    let html = render_html(&doc);
    assert!(!html.contains("<script>"));
    assert!(html.contains("hello"));
}

#[test]
fn sanitize_onerror() {
    let sanitized = penumbra_markdown::render::html::sanitize("<img src=x onerror=alert(1)>");
    assert!(
        !sanitized.contains("onerror"),
        "sanitized contains onerror: {sanitized}"
    );
}

#[test]
fn sanitize_keeps_penumbra() {
    let sanitized =
        penumbra_markdown::render::html::sanitize("<pe-embed data-ref=\"x\"></pe-embed>");
    assert!(
        sanitized.contains("<pe-embed"),
        "sanitized missing pe-embed: {sanitized}"
    );
}

#[test]
fn render_strike() {
    let doc = parse_document("~~done~~").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<del>done</del>"));
}

#[test]
fn render_table_tag() {
    let doc = parse_document("| H1 |\n|---|\n| C1 |").unwrap();
    let html = render_html(&doc);
    assert!(html.contains("<table>"));
    assert!(html.contains("<th>"));
    assert!(html.contains("<td>"));
}

// Text render tests

#[test]
fn plain_text_heading() {
    let doc = parse_document("# Title").unwrap();
    assert_eq!(render_plain(&doc), "Title");
}

#[test]
fn plain_text_paragraph() {
    let doc = parse_document("Hello world").unwrap();
    assert_eq!(render_plain(&doc), "Hello world");
}

#[test]
fn plain_text_code_block() {
    let doc = parse_document("```\ncode here\n```").unwrap();
    assert_eq!(render_plain(&doc).trim(), "code here");
}

#[test]
fn plain_text_embed_is_included() {
    let doc = parse_document("ref [[some-id]]").unwrap();
    assert_eq!(render_plain(&doc), "ref some-id");
}

#[test]
fn plain_text_tag_is_included() {
    let doc = parse_document("tag #hello").unwrap();
    assert_eq!(render_plain(&doc), "tag hello");
}

// Stream tests

#[test]
fn stream_empty_initial() {
    let stream = penumbra_markdown::stream::MarkdownStream::new();
    assert!(stream.snapshot().is_empty());
}

#[test]
fn stream_append_para() {
    let mut stream = penumbra_markdown::stream::MarkdownStream::new();
    let update = stream.append("hello world\n\n").unwrap();
    assert!(!update.committed.is_empty() || update.pending.is_some());
}

#[test]
fn stream_finalize_flushes() {
    let mut stream = penumbra_markdown::stream::MarkdownStream::new();
    stream.append("incomplete").unwrap();
    stream.finalize().unwrap();
    assert!(!stream.snapshot().is_empty());
}

#[test]
fn stream_reset_clears() {
    let mut stream = penumbra_markdown::stream::MarkdownStream::new();
    stream.append("hello\n\n").unwrap();
    stream.reset();
    assert!(stream.snapshot().is_empty());
}

#[test]
fn stream_snapshot_accumulates() {
    let mut stream = penumbra_markdown::stream::MarkdownStream::new();
    stream.append("# A\n\n").unwrap();
    stream.append("# B\n\n").unwrap();
    let snap1 = stream.snapshot();
    stream.append("# C\n\n").unwrap();
    let snap2 = stream.snapshot();
    assert!(snap2.blocks.len() >= snap1.blocks.len());
}

fn sample_frontmatter() -> Frontmatter {
    Frontmatter {
        id: Uuid::parse_str("fb5eb8b9-6808-4544-9f1b-a8b5d0726b3d").unwrap(),
        created_at: Utc.with_ymd_and_hms(2026, 8, 25, 4, 4, 18).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2026, 8, 26, 10, 0, 0).unwrap(),
        tags: vec!["project".to_string(), "two words".to_string()],
        pinned: true,
        archived: false,
    }
}

#[test]
fn frontmatter_roundtrip_preserves_fields() {
    let fm = sample_frontmatter();
    let text = serialize_fm(&fm).unwrap() + "Some body\n";
    let parsed = parse_fm(&text).unwrap();
    assert_eq!(parsed.frontmatter, Some(fm));
    assert_eq!(parsed.body, "Some body\n");
}

#[test]
fn file_without_frontmatter_parses_as_body_only() {
    let parsed = parse_fm("just markdown\n").unwrap();
    assert!(parsed.frontmatter.is_none());
    assert_eq!(parsed.body, "just markdown\n");
}

#[test]
fn unknown_keys_are_ignored_on_parse() {
    let text = "---\nid: fb5eb8b9-6808-4544-9f1b-a8b5d0726b3d\ncreated: 2026-08-25T04:04:18Z\nupdated: 2026-08-25T04:04:18Z\ncustom: stuff\n---\nbody\n";
    assert!(parse_fm(text).unwrap().frontmatter.is_some());
}

#[test]
fn unterminated_block_is_an_error() {
    let text = "---\nid: fb5eb8b9-6808-4544-9f1b-a8b5d0726b3d\n";
    assert!(parse_fm(text).is_err());
}

#[test]
fn quoted_tags_roundtrip_through_list_parser() {
    let text = "---\nid: fb5eb8b9-6808-4544-9f1b-a8b5d0726b3d\ncreated: 2026-08-25T04:04:18Z\nupdated: 2026-08-25T04:04:18Z\ntags: [plain, \"has, comma\", \"quote\\\"d\"]\n---\n\n";
    let fm = parse_fm(text).unwrap().frontmatter.unwrap();
    assert_eq!(
        fm.tags,
        vec![
            "plain".to_string(),
            "has, comma".to_string(),
            "quote\"d".to_string()
        ]
    );
}

#[test]
fn byte_order_mark_is_stripped_before_fence_check() {
    let text = format!("\u{feff}{}", serialize_fm(&sample_frontmatter()).unwrap());
    assert!(parse_fm(&text).unwrap().frontmatter.is_some());
}

#[test]
fn default_flags_are_omitted_from_output() {
    let mut fm = sample_frontmatter();
    fm.tags = Vec::new();
    fm.pinned = false;
    let text = serialize_fm(&fm).unwrap();
    assert!(!text.contains("tags"));
    assert!(!text.contains("pinned"));
}

#[test]
fn malformed_timestamp_is_rejected() {
    let text = "---\nid: fb5eb8b9-6808-4544-9f1b-a8b5d0726b3d\ncreated: not-a-time\nupdated: 2026-08-25T04:04:18Z\n---\n";
    assert!(parse_fm(text).is_err());
}

// Wikilink and inline tag extraction tests

use penumbra_markdown::links::{
    extract_inline_tags, extract_wikilinks, normalize_link_target, rewrite_wikilink_targets,
};

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
