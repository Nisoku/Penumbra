use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    Dark,
    Light,
    System,
}

impl ThemeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }
}

/// A complete set of design tokens. Every UI surface references these by their
/// CSS-variable name (see [`Theme::to_css_vars`]), so swapping the `Theme`
/// re-skins the whole app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub mode: ThemeMode,

    // Surfaces
    pub bg: String,
    pub bg_2: String,
    pub bg_elevated: String,
    pub bg_overlay: String,
    pub bg_input: String,

    // Lines
    pub border: String,
    pub border_strong: String,

    // Text
    pub text: String,
    pub text_dim: String,
    pub text_faint: String,

    // Accent
    pub accent: String,
    pub accent_bright: String,
    pub accent_soft: String,
    pub accent_strong: String,

    // Semantic
    pub danger: String,
    pub danger_soft: String,
    pub pin: String,

    // Canvas
    pub grid_dot: String,
    pub edge: String,
    pub edge_strong: String,

    // Effects
    pub shadow_sm: String,
    pub shadow_lg: String,
    pub glass_blur: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            bg: "#0a0e1a".into(),
            bg_2: "#0e1322".into(),
            bg_elevated: "rgba(22, 30, 52, 0.78)".into(),
            bg_overlay: "rgba(16, 22, 40, 0.97)".into(),
            bg_input: "rgba(120, 160, 230, 0.07)".into(),

            border: "rgba(120, 160, 230, 0.13)".into(),
            border_strong: "rgba(120, 160, 230, 0.30)".into(),

            text: "#e3ecfb".into(),
            text_dim: "rgba(200, 216, 245, 0.64)".into(),
            text_faint: "rgba(165, 188, 232, 0.40)".into(),

            accent: "#6b9be0".into(),
            accent_bright: "#93c1ff".into(),
            accent_soft: "rgba(110, 155, 224, 0.15)".into(),
            accent_strong: "#3b82e0".into(),

            danger: "#ff7d7d".into(),
            danger_soft: "rgba(255, 105, 105, 0.14)".into(),
            pin: "#ffc16b".into(),

            grid_dot: "rgba(120, 160, 230, 0.15)".into(),
            edge: "rgba(120, 160, 230, 0.50)".into(),
            edge_strong: "rgba(147, 193, 255, 0.92)".into(),

            shadow_sm: "0 2px 14px rgba(0, 0, 0, 0.35)".into(),
            shadow_lg: "0 16px 48px rgba(0, 0, 0, 0.55), 0 0 0 0.5px rgba(120, 160, 230, 0.10)"
                .into(),
            glass_blur: "14px".into(),
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            bg: "#f4f6fb".into(),
            bg_2: "#e6ecf7".into(),
            bg_elevated: "rgba(255, 255, 255, 0.90)".into(),
            bg_overlay: "rgba(255, 255, 255, 0.98)".into(),
            bg_input: "rgba(45, 95, 165, 0.06)".into(),

            border: "rgba(45, 95, 165, 0.15)".into(),
            border_strong: "rgba(45, 95, 165, 0.32)".into(),

            text: "#14233f".into(),
            text_dim: "rgba(30, 55, 100, 0.68)".into(),
            text_faint: "rgba(30, 55, 100, 0.42)".into(),

            accent: "#3672bf".into(),
            accent_bright: "#245fb0".into(),
            accent_soft: "rgba(45, 110, 200, 0.10)".into(),
            accent_strong: "#2f7be8".into(),

            danger: "#d63d40".into(),
            danger_soft: "rgba(214, 61, 64, 0.10)".into(),
            pin: "#cf8a2b".into(),

            grid_dot: "rgba(45, 95, 165, 0.16)".into(),
            edge: "rgba(45, 95, 165, 0.42)".into(),
            edge_strong: "rgba(36, 95, 176, 0.85)".into(),

            shadow_sm: "0 2px 14px rgba(40, 70, 120, 0.12)".into(),
            shadow_lg: "0 16px 48px rgba(40, 70, 120, 0.20), 0 0 0 0.5px rgba(45, 95, 165, 0.08)"
                .into(),
            glass_blur: "16px".into(),
        }
    }

    pub fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            _ => Self::dark(),
        }
    }

    /// The token declarations, e.g. `--bg: #0a0e1a; --text: ...;`.
    pub fn to_css_vars(&self) -> String {
        format!(
            "--bg: {bg};\
             --bg-2: {bg_2};\
             --bg-elevated: {bg_elevated};\
             --bg-overlay: {bg_overlay};\
             --bg-input: {bg_input};\
             --border: {border};\
             --border-strong: {border_strong};\
             --text: {text};\
             --text-dim: {text_dim};\
             --text-faint: {text_faint};\
             --accent: {accent};\
             --accent-bright: {accent_bright};\
             --accent-soft: {accent_soft};\
             --accent-strong: {accent_strong};\
             --danger: {danger};\
             --danger-soft: {danger_soft};\
             --pin: {pin};\
             --grid-dot: {grid_dot};\
             --edge: {edge};\
             --edge-strong: {edge_strong};\
             --shadow-sm: {shadow_sm};\
             --shadow-lg: {shadow_lg};\
             --glass-blur: {glass_blur};",
            bg = self.bg,
            bg_2 = self.bg_2,
            bg_elevated = self.bg_elevated,
            bg_overlay = self.bg_overlay,
            bg_input = self.bg_input,
            border = self.border,
            border_strong = self.border_strong,
            text = self.text,
            text_dim = self.text_dim,
            text_faint = self.text_faint,
            accent = self.accent,
            accent_bright = self.accent_bright,
            accent_soft = self.accent_soft,
            accent_strong = self.accent_strong,
            danger = self.danger,
            danger_soft = self.danger_soft,
            pin = self.pin,
            grid_dot = self.grid_dot,
            edge = self.edge,
            edge_strong = self.edge_strong,
            shadow_sm = self.shadow_sm,
            shadow_lg = self.shadow_lg,
            glass_blur = self.glass_blur,
        )
    }

    /// A full `:root { … }` block ready to drop into a `<style>` element.
    pub fn css_root_block(&self) -> String {
        format!(":root {{ {} }}", self.to_css_vars())
    }
}

/// Theme-agnostic global polish: fonts, scrollbars, selection, and the
/// smooth cross-fade when the theme changes. References the tokens above.
pub const BASE_CSS: &str = r#"
html, body {
    background: var(--bg);
    color: var(--text);
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    font-family: -apple-system, BlinkMacSystemFont, "Inter", "Segoe UI", Roboto, sans-serif;
    -webkit-user-select: none;
    user-select: none;
    cursor: default;
}
body, .pnb-themed { transition: background-color 240ms ease, color 240ms ease; }
::selection { background: var(--accent-soft); color: var(--accent-bright); }
* { scrollbar-width: thin; scrollbar-color: var(--border-strong) transparent; }
*::-webkit-scrollbar { width: 9px; height: 9px; }
*::-webkit-scrollbar-track { background: transparent; }
*::-webkit-scrollbar-thumb {
    background: var(--border-strong);
    border-radius: 6px;
    border: 2px solid transparent;
    background-clip: content-box;
}
*::-webkit-scrollbar-thumb:hover { background: var(--accent); background-clip: content-box; }
input, textarea, [contenteditable], .ProseMirror, .pnb-selectable {
    -webkit-user-select: text;
    user-select: text;
}
"#;
