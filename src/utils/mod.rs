pub fn init_logger() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
}
