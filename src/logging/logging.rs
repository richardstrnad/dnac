use std::fs::OpenOptions;
use std::sync::Arc;
use tracing_subscriber::{prelude::*, EnvFilter};

pub fn init_logging() {
    let stdout_log = tracing_subscriber::fmt::layer();

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open("debug.log")
        .unwrap();
    let debug_log = tracing_subscriber::fmt::layer().with_writer(Arc::new(file));

    tracing_subscriber::registry()
        .with(
            stdout_log
                .with_filter(EnvFilter::from_default_env())
                .and_then(debug_log),
        )
        .init();
}
