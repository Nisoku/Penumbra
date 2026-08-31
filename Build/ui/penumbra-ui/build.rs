use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let config = slint_build::CompilerConfiguration::new().with_library_paths(HashMap::from([(
        "penumbra-component".to_owned(),
        PathBuf::from(penumbra_component::SLINT_LIBRARY_PATH),
    )]));
    slint_build::compile_with_config("ui/app.slint", config).expect("slint compilation failed");
}
