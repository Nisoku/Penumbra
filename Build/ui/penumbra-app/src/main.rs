use dioxus_desktop::{Config, LogicalSize, WindowBuilder};
use penumbra_app::App;

fn main() {
    let theme_css = include_str!("../assets/dx-components-theme.css");
    let reset_css = "\
        *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }\
        html, body { margin: 0; height: 100vh; overflow: hidden; }\
        #main { height: 100vh; width: 100vw; }\
    ";
    let window = WindowBuilder::new()
        .with_title("Penumbra")
        .with_inner_size(LogicalSize::new(1280.0, 800.0));
    let config = Config::new()
        .with_window(window)
        .with_background_color((10, 15, 30, 255))
        .with_custom_head(format!("<style>{reset_css} {theme_css}</style>"));
    dioxus_desktop::launch::launch(App, vec![], vec![Box::new(config)]);
}
