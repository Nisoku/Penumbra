use penumbra_markdown::parser::parse_document;
use penumbra_markdown::render::html::render_html;
use penumbra_markdown::render::text::render_plain;

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
