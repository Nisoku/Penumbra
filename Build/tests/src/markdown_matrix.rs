use penumbra_markdown::ast::Block;
use penumbra_markdown::ast::Document;
use penumbra_markdown::ast::Inline;
use penumbra_markdown::ast::Inline::*;
use penumbra_markdown::parser::markdown_to_html;
use penumbra_markdown::parser::markdown_to_plain;
use penumbra_markdown::parser::parse_document;
use penumbra_markdown::render::html::render_html;
use penumbra_markdown::render::text::render_plain;

// Parser tests

fn assert_para(doc: &Document, idx: usize, expected: &[Inline]) {
    match &doc.blocks[idx] {
        Block::Paragraph(children) => {
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
    match &doc.blocks[0] {
        Block::Heading { level, children } => {
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
        match &doc.blocks[0] {
            Block::Heading { level, .. } => assert_eq!(*level, want),
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
    match &doc.blocks[0] {
        Block::Heading { level, .. } => assert_eq!(*level, 2),
        other => panic!("expected Heading, got {:?}", other),
    }
}

#[test]
fn parser_fenced_code() {
    let doc = parse_document("```rust\nfn main() {}\n```").unwrap();
    match &doc.blocks[0] {
        Block::CodeBlock { language, text } => {
            assert_eq!(language.as_deref(), Some("rust"));
            assert!(text.contains("fn main()"));
        }
        other => panic!("expected CodeBlock, got {:?}", other),
    }
}

#[test]
fn parser_indented_code() {
    let doc = parse_document("    code line").unwrap();
    match &doc.blocks[0] {
        Block::CodeBlock { language, text } => {
            assert!(language.is_none());
            assert_eq!(text.trim(), "code line");
        }
        other => panic!("expected CodeBlock, got {:?}", other),
    }
}

#[test]
fn parser_strong_em() {
    let doc = parse_document("**bold** and *italic*").unwrap();
    match &doc.blocks[0] {
        Block::Paragraph(children) => {
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
    match &doc.blocks[0] {
        Block::Paragraph(children) => match &children[0] {
            Strikethrough(c) => assert_eq!(c, &[Text("struck".into())]),
            other => panic!("expected Strikethrough, got {:?}", other),
        },
        other => panic!("expected Paragraph, got {:?}", other),
    }
}

#[test]
fn parser_link() {
    let doc = parse_document("[text](http://example.com)").unwrap();
    match &doc.blocks[0] {
        Block::Paragraph(children) => {
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
    match &doc.blocks[0] {
        Block::Paragraph(children) => match &children[0] {
            Link { title, .. } => assert!(title.contains("Title")),
            other => panic!("expected Link, got {:?}", other),
        },
        other => panic!("expected Paragraph, got {:?}", other),
    }
}

#[test]
fn parser_image() {
    let doc = parse_document("![alt](http://ex.com/img.png)").unwrap();
    match &doc.blocks[0] {
        Block::Paragraph(children) => match &children[0] {
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
    match &doc.blocks[0] {
        Block::List { ordered, items, .. } => {
            assert!(!ordered);
            assert_eq!(items.len(), 2);
        }
        other => panic!("expected List, got {:?}", other),
    }
}

#[test]
fn parser_olist() {
    let doc = parse_document("1. first\n2. second").unwrap();
    match &doc.blocks[0] {
        Block::List { ordered, items, .. } => {
            assert!(ordered);
            assert_eq!(items.len(), 2);
        }
        other => panic!("expected List, got {:?}", other),
    }
}

#[test]
fn parser_quote() {
    let doc = parse_document("> quoted text").unwrap();
    match &doc.blocks[0] {
        Block::Quote(children) => assert!(!children.is_empty()),
        other => panic!("expected Quote, got {:?}", other),
    }
}

#[test]
fn parser_hr() {
    let doc = parse_document("---\n").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    assert_eq!(doc.blocks[0], Block::ThematicBreak);
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
    assert!(matches!(doc.blocks[0], Block::Heading { level: 1, .. }));
    assert!(matches!(doc.blocks[1], Block::Paragraph(_)));
    assert!(matches!(doc.blocks[2], Block::List { .. }));
}

#[test]
fn parser_table() {
    let doc = parse_document("| H1 | H2 |\n|---|---|\n| C1 | C2 |").unwrap();
    assert!(!doc.blocks.is_empty());
    match &doc.blocks[0] {
        Block::Table(t) => {
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
    match &doc.blocks[0] {
        Block::List { items, .. } => assert_eq!(items.len(), 2),
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
    match &doc.blocks[0] {
        Block::Paragraph(children) => {
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
