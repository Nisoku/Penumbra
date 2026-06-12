use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    Dark,
    Light,
    System,
}

impl ThemeMode {
    pub fn to_css_class(&self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub mode: ThemeMode,
    pub colors: ThemeColors,
    pub radii: ThemeRadii,
    pub glass: GlassConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    pub bg_canvas: String,
    pub bg_card: String,
    pub bg_sidebar: String,
    pub bg_dialog: String,
    pub text_primary: String,
    pub text_secondary: String,
    pub text_muted: String,
    pub accent: String,
    pub accent_hover: String,
    pub danger: String,
    pub success: String,
    pub card_border: String,
    pub card_shadow: String,
    pub grid_color: String,
    pub link_color: String,
    pub link_hover_color: String,
    pub scrollbar_bg: String,
    pub scrollbar_thumb: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThemeRadii {
    pub card: f64,
    pub button: f64,
    pub panel: f64,
    pub input: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GlassConfig {
    pub blur_radius: f64,
    pub saturation: f64,
    pub opacity: f64,
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
            colors: ThemeColors {
                bg_canvas: "#0f0f1a".into(),
                bg_card: "rgba(255, 255, 255, 0.04)".into(),
                bg_sidebar: "rgba(15, 15, 26, 0.95)".into(),
                bg_dialog: "#1a1a2e".into(),
                text_primary: "#e8e8f0".into(),
                text_secondary: "#9090a0".into(),
                text_muted: "#505060".into(),
                accent: "#7c5cbf".into(),
                accent_hover: "#9b7ed4".into(),
                danger: "#e74c3c".into(),
                success: "#2ecc71".into(),
                card_border: "rgba(255, 255, 255, 0.06)".into(),
                card_shadow: "0 4px 24px rgba(0, 0, 0, 0.4)".into(),
                grid_color: "rgba(255, 255, 255, 0.04)".into(),
                link_color: "rgba(124, 92, 191, 0.3)".into(),
                link_hover_color: "rgba(124, 92, 191, 0.6)".into(),
                scrollbar_bg: "transparent".into(),
                scrollbar_thumb: "rgba(255, 255, 255, 0.1)".into(),
            },
            radii: ThemeRadii {
                card: 12.0,
                button: 8.0,
                panel: 8.0,
                input: 6.0,
            },
            glass: GlassConfig {
                blur_radius: 12.0,
                saturation: 1.8,
                opacity: 0.85,
            },
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            colors: ThemeColors {
                bg_canvas: "#f5f5fa".into(),
                bg_card: "rgba(255, 255, 255, 0.8)".into(),
                bg_sidebar: "rgba(245, 245, 250, 0.95)".into(),
                bg_dialog: "#ffffff".into(),
                text_primary: "#1a1a2e".into(),
                text_secondary: "#606078".into(),
                text_muted: "#a0a0b0".into(),
                accent: "#7c5cbf".into(),
                accent_hover: "#6a4da8".into(),
                danger: "#e74c3c".into(),
                success: "#27ae60".into(),
                card_border: "rgba(0, 0, 0, 0.06)".into(),
                card_shadow: "0 4px 24px rgba(0, 0, 0, 0.08)".into(),
                grid_color: "rgba(0, 0, 0, 0.04)".into(),
                link_color: "rgba(124, 92, 191, 0.15)".into(),
                link_hover_color: "rgba(124, 92, 191, 0.4)".into(),
                scrollbar_bg: "transparent".into(),
                scrollbar_thumb: "rgba(0, 0, 0, 0.1)".into(),
            },
            radii: ThemeRadii {
                card: 12.0,
                button: 8.0,
                panel: 8.0,
                input: 6.0,
            },
            glass: GlassConfig {
                blur_radius: 12.0,
                saturation: 1.5,
                opacity: 0.9,
            },
        }
    }

    pub fn to_css_vars(&self) -> String {
        let c = &self.colors;
        let r = &self.radii;
        let g = &self.glass;
        format!(
            r#"--bg-canvas: {bg_canvas};
--bg-card: {bg_card};
--bg-sidebar: {bg_sidebar};
--bg-dialog: {bg_dialog};
--text-primary: {text_primary};
--text-secondary: {text_secondary};
--text-muted: {text_muted};
--accent: {accent};
--accent-hover: {accent_hover};
--danger: {danger};
--success: {success};
--card-border: {card_border};
--card-shadow: {card_shadow};
--grid-color: {grid_color};
--link-color: {link_color};
--link-hover-color: {link_hover_color};
--scrollbar-bg: {scrollbar_bg};
--scrollbar-thumb: {scrollbar_thumb};
--radius-card: {radius_card}px;
--radius-button: {radius_button}px;
--radius-panel: {radius_panel}px;
--radius-input: {radius_input}px;
--glass-blur: {glass_blur}px;
--glass-saturation: {glass_saturation};
--glass-opacity: {glass_opacity};"#,
            bg_canvas = c.bg_canvas,
            bg_card = c.bg_card,
            bg_sidebar = c.bg_sidebar,
            bg_dialog = c.bg_dialog,
            text_primary = c.text_primary,
            text_secondary = c.text_secondary,
            text_muted = c.text_muted,
            accent = c.accent,
            accent_hover = c.accent_hover,
            danger = c.danger,
            success = c.success,
            card_border = c.card_border,
            card_shadow = c.card_shadow,
            grid_color = c.grid_color,
            link_color = c.link_color,
            link_hover_color = c.link_hover_color,
            scrollbar_bg = c.scrollbar_bg,
            scrollbar_thumb = c.scrollbar_thumb,
            radius_card = r.card,
            radius_button = r.button,
            radius_panel = r.panel,
            radius_input = r.input,
            glass_blur = g.blur_radius,
            glass_saturation = g.saturation,
            glass_opacity = g.opacity,
        )
    }
}
