use cosmic_text::fontdb::{Database, Source};
use cosmic_text::FontSystem;
use std::sync::Arc;

// Inter (sans) and JetBrains Mono (mono)
const INTER: &[u8] = include_bytes!("../../assets/fonts/InterVariable.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");

const SANS_FAMILY: &str = "Inter";
const MONO_FAMILY: &str = "JetBrains Mono";

/// Build a `FontSystem` using those ones
pub(super) fn build_font_system() -> FontSystem {
    let mut db = Database::new();
    db.load_font_source(Source::Binary(Arc::new(INTER)));
    db.load_font_source(Source::Binary(Arc::new(JETBRAINS_MONO)));

    db.set_sans_serif_family(SANS_FAMILY);
    db.set_serif_family(SANS_FAMILY);
    db.set_cursive_family(SANS_FAMILY);
    db.set_fantasy_family(SANS_FAMILY);
    db.set_monospace_family(MONO_FAMILY);

    FontSystem::new_with_locale_and_db("en-US".to_string(), db)
}
