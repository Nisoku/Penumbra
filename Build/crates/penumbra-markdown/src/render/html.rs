use std::collections::{HashMap, HashSet};

use ammonia::Builder;

use crate::ast::{Block, Document, Inline, ListItem, Table, TableAlign};

pub fn render_html(doc: &Document) -> String {
    let raw = render_doc(doc);
    sanitize(&raw)
}

pub fn sanitize(raw: &str) -> String {
    let tags: HashSet<&str> = [
        "a",
        "abbr",
        "b",
        "blockquote",
        "br",
        "caption",
        "cite",
        "code",
        "col",
        "colgroup",
        "dd",
        "del",
        "details",
        "dfn",
        "div",
        "dl",
        "dt",
        "em",
        "figcaption",
        "figure",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "i",
        "img",
        "ins",
        "kbd",
        "li",
        "mark",
        "ol",
        "p",
        "pre",
        "q",
        "s",
        "samp",
        "small",
        "span",
        "strong",
        "sub",
        "summary",
        "sup",
        "table",
        "tbody",
        "td",
        "tfoot",
        "th",
        "thead",
        "time",
        "tr",
        "u",
        "ul",
        "var",
        "pe-embed",
        "pe-tag",
    ]
    .into();

    let attrs: HashMap<&str, HashSet<&str>> = [
        ("pe-embed", ["data-ref"].into()),
        ("pe-tag", ["data-name"].into()),
        ("a", ["href", "title", "target"].into()),
        ("img", ["src", "alt", "title", "width", "height"].into()),
        ("td", ["align"].into()),
        ("th", ["align"].into()),
        ("code", ["class"].into()),
        ("pre", ["class"].into()),
        ("details", ["open"].into()),
        ("ol", ["start", "reversed"].into()),
        ("li", ["checked"].into()),
    ]
    .into();

    Builder::default()
        .tags(tags)
        .tag_attributes(attrs)
        .link_rel(Some("noopener noreferrer"))
        .clean(raw)
        .to_string()
}

fn render_doc(doc: &Document) -> String {
    let mut buf = String::new();
    for block in &doc.blocks {
        render_block(block, &mut buf);
    }
    buf
}

fn render_block(block: &Block, buf: &mut String) {
    match block {
        Block::Paragraph(children) => {
            buf.push_str("<p>");
            render_inlines(children, buf);
            buf.push_str("</p>\n");
        }
        Block::Heading { level, children } => {
            buf.push_str(&format!("<h{}>", level));
            render_inlines(children, buf);
            buf.push_str(&format!("</h{}>\n", level));
        }
        Block::CodeBlock { language, text } => {
            if let Some(lang) = language {
                buf.push_str(&format!(
                    "<pre><code class=\"language-{}\">{}</code></pre>\n",
                    escape_html(lang),
                    escape_html(text),
                ));
            } else {
                buf.push_str(&format!("<pre><code>{}</code></pre>\n", escape_html(text),));
            }
        }
        Block::List {
            ordered,
            start,
            items,
        } => {
            if *ordered {
                if let Some(s) = start {
                    buf.push_str(&format!("<ol start=\"{}\">\n", s));
                } else {
                    buf.push_str("<ol>\n");
                }
            } else {
                buf.push_str("<ul>\n");
            }
            for item in items {
                render_list_item(item, buf);
            }
            if *ordered {
                buf.push_str("</ol>\n");
            } else {
                buf.push_str("</ul>\n");
            }
        }
        Block::Quote(children) => {
            buf.push_str("<blockquote>\n");
            for child in children {
                render_block(child, buf);
            }
            buf.push_str("</blockquote>\n");
        }
        Block::ThematicBreak => buf.push_str("<hr>\n"),
        Block::Table(table) => render_table(table, buf),
        Block::HtmlBlock(html) => {
            buf.push_str(html);
            buf.push('\n');
        }
        Block::FootnoteDefinition { name, children } => {
            buf.push_str(&format!(
                "<div class=\"footnote-definition\" id=\"{}\">\n",
                escape_html(name),
            ));
            for child in children {
                render_block(child, buf);
            }
            buf.push_str("</div>\n");
        }
    }
}

fn render_list_item(item: &ListItem, buf: &mut String) {
    buf.push_str("<li");
    if let Some(checked) = item.checked {
        if checked {
            buf.push_str(" checked=\"checked\"");
        } else {
            buf.push_str(" checked=\"false\"");
        }
    }
    buf.push('>');
    for child in &item.children {
        render_block(child, buf);
    }
    buf.push_str("</li>\n");
}

fn render_table(table: &Table, buf: &mut String) {
    buf.push_str("<table>\n");

    if !table.headers.is_empty() {
        buf.push_str("<thead>\n<tr>\n");
        for (i, cell) in table.headers.iter().enumerate() {
            let a = table.align.get(i).and_then(|a| match a {
                TableAlign::Left => Some(" left"),
                TableAlign::Center => Some(" center"),
                TableAlign::Right => Some(" right"),
                TableAlign::None => None,
            });
            if let Some(align) = a {
                buf.push_str(&format!("<th align=\"{}\">", align.trim()));
            } else {
                buf.push_str("<th>");
            }
            render_inlines(cell, buf);
            buf.push_str("</th>");
        }
        buf.push_str("\n</tr>\n</thead>\n");
    }

    if !table.rows.is_empty() {
        buf.push_str("<tbody>\n");
        for row in &table.rows {
            buf.push_str("<tr>\n");
            for (i, cell) in row.iter().enumerate() {
                let a = table.align.get(i).and_then(|a| match a {
                    TableAlign::Left => Some(" left"),
                    TableAlign::Center => Some(" center"),
                    TableAlign::Right => Some(" right"),
                    TableAlign::None => None,
                });
                if let Some(align) = a {
                    buf.push_str(&format!("<td align=\"{}\">", align.trim()));
                } else {
                    buf.push_str("<td>");
                }
                render_inlines(cell, buf);
                buf.push_str("</td>");
            }
            buf.push_str("\n</tr>\n");
        }
        buf.push_str("</tbody>\n");
    }

    buf.push_str("</table>\n");
}

fn render_inlines(inlines: &[Inline], buf: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => buf.push_str(&escape_html(text)),
            Inline::Strong(children) => {
                buf.push_str("<strong>");
                render_inlines(children, buf);
                buf.push_str("</strong>");
            }
            Inline::Emphasis(children) => {
                buf.push_str("<em>");
                render_inlines(children, buf);
                buf.push_str("</em>");
            }
            Inline::Strikethrough(children) => {
                buf.push_str("<del>");
                render_inlines(children, buf);
                buf.push_str("</del>");
            }
            Inline::Code(code) => {
                buf.push_str("<code>");
                buf.push_str(&escape_html(code));
                buf.push_str("</code>");
            }
            Inline::Link {
                url,
                title,
                children,
            } => {
                buf.push_str(&format!("<a href=\"{}\"", escape_html(url)));
                if !title.is_empty() {
                    buf.push_str(&format!(" title=\"{}\"", escape_html(title)));
                }
                buf.push('>');
                render_inlines(children, buf);
                buf.push_str("</a>");
            }
            Inline::Image { url, alt, title } => {
                buf.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\"",
                    escape_html(url),
                    escape_html(alt),
                ));
                if !title.is_empty() {
                    buf.push_str(&format!(" title=\"{}\"", escape_html(title)));
                }
                buf.push('>');
            }
            Inline::NoteEmbed { note_ref } => {
                buf.push_str(&format!(
                    "<pe-embed data-ref=\"{}\"></pe-embed>",
                    escape_html(note_ref),
                ));
            }
            Inline::TagRef { name } => {
                buf.push_str(&format!(
                    "<pe-tag data-name=\"{}\"></pe-tag>",
                    escape_html(name),
                ));
            }
            Inline::LineBreak => buf.push_str("<br>\n"),
            Inline::SoftBreak => buf.push('\n'),
        }
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
