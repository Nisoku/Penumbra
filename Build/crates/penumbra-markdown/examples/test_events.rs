fn main() {
    let text = "[[only]]";
    let opts = pulldown_cmark::Options::ENABLE_TABLES
        | pulldown_cmark::Options::ENABLE_FOOTNOTES
        | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
        | pulldown_cmark::Options::ENABLE_TASKLISTS
        | pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES;
    for event in pulldown_cmark::Parser::new_ext(text, opts) {
        eprintln!("{:?}", event);
    }
}
