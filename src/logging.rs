use color_eyre::eyre::Context;
use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};
use std::path::PathBuf;
use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};

pub fn setup_logging() -> color_eyre::Result<WorkerGuard> {
    let path = PathBuf::from("logs/mizer.log");
    let file_appender = BasicRollingFileAppender::new(
        path,
        RollingConditionBasic::new()
            .daily()
            .max_size(1024 * 1024 * 2),
        4,
    )
    .context("Creating tracing file appender")?;
    let (file_appender, guard) = NonBlocking::new(file_appender);
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .with_writer(file_appender)
        .init();

    Ok(guard)
}
