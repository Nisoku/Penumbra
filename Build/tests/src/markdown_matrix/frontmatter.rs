use chrono::{TimeZone, Utc};
use penumbra_markdown::frontmatter::{parse as parse_fm, serialize as serialize_fm, Frontmatter};
use uuid::Uuid;

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
