pub mod config;
pub mod storage;

pub fn init_logger() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();
}

pub fn init() {
    init_logger();
    log::info!("Penumbra initialized");
}