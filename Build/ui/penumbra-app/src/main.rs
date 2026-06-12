use dioxus_desktop::{Config, LogicalSize, WindowBuilder};
use penumbra_app::App;

fn main() {
    let components_css = include_str!("../assets/dx-components-theme.css");
    // Default (dark) tokens + global polish live in the penumbra-theme crate.
    // The running app re-emits the active theme's tokens into a <style> so the
    // theme toggle works; this just guarantees correct colors before first paint.
    let theme = penumbra_theme::Theme::dark();
    let theme_css = format!("{}\n{}", theme.css_root_block(), penumbra_theme::BASE_CSS);
    let reset_css = "\
        *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }\
        html, body { margin: 0; height: 100vh; overflow: hidden; }\
        #main { height: 100vh; width: 100vw; }\
    ";
    // Suppress the native macOS / WebView right-click menu so our own context
    // menu can take over everywhere.
    let suppress_native_menu = "\
        <script>\
        window.addEventListener('contextmenu', function(e){ e.preventDefault(); }, false);\
        window.addEventListener('dragstart', function(e){ e.preventDefault(); }, false);\
        </script>\
    ";
    let window = WindowBuilder::new()
        .with_title("Penumbra")
        .with_inner_size(LogicalSize::new(1280.0, 800.0));
    let config = Config::new()
        .with_window(window)
        .with_background_color((10, 14, 26, 255))
        .with_custom_head(format!(
            "<style>{reset_css}\n{components_css}\n{theme_css}</style>{suppress_native_menu}"
        ));
    dioxus_desktop::launch::launch(App, vec![], vec![Box::new(config)]);
}
