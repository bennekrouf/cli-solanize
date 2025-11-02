#[macro_export]
macro_rules! init_logging {
    ($format:expr, $file_path:expr, $service:expr, $component:expr) => {{
        use std::fs::OpenOptions;
        use tracing_subscriber::{EnvFilter, fmt};

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open($file_path)
            .expect("Failed to open log file");

        match $format.as_str() {
            "json" => {
                tracing_subscriber::registry()
                    .with(
                        fmt::layer()
                            .json()
                            .with_writer(file)
                            .with_current_span(false)
                            .with_span_list(false),
                    )
                    .with(
                        EnvFilter::from_default_env()
                            .add_directive("trace".parse().expect("Invalid log directive")),
                    )
                    .init();
            }
            _ => {
                tracing_subscriber::registry()
                    .with(fmt::layer().pretty().with_writer(file))
                    .with(
                        EnvFilter::from_default_env()
                            .add_directive("trace".parse().expect("Invalid log directive")),
                    )
                    .init();
            }
        }
    }};
}

#[macro_export]
macro_rules! app_log {
    ($level:ident, $($arg:tt)*) => {
        tracing::$level!(service = env!("CARGO_PKG_NAME"), component = "main", $($arg)*)
    };
    ($level:ident, $service:expr, $component:expr, $($arg:tt)*) => {
        tracing::$level!(service = $service, component = $component, $($arg)*)
    };
}

