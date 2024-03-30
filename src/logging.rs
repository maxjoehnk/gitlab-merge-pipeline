use std::path::PathBuf;

use color_eyre::eyre::Context;
use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};
use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn setup_logging() -> color_eyre::Result<WorkerGuard> {
    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("gitlab_merge_pipeline", LevelFilter::DEBUG)
        .with_default(LevelFilter::INFO);
    let path = PathBuf::from("logs/gitlab-merge-pipeline.log");
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
        .with_writer(file_appender)
        .finish()
        .with(filter)
        .init();

    Ok(guard)
}
