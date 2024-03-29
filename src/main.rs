use std::time::Duration;

use crate::api::*;
use crate::logging::setup_logging;
use crate::stages::*;

use self::merge_queue::*;

mod api;
mod config;
mod logging;
mod merge_queue;
mod stages;

#[tokio::main(flavor = "current_thread")]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let _guard = setup_logging()?;

    let config = config::Config::read().await?;

    let gitlab_client = build_client(&config).await?;

    for repository in config.repositories {
        // TODO: spawn task per repository
        let mut queue = MergeQueue::default();

        loop {
            fetch_merge_requests(&gitlab_client, &repository, &mut queue).await?;
            if !queue
                .merge_requests
                .iter()
                .any(|mr| mr.state == QueueState::Running)
            {
                if let Some(next_task) = queue
                    .merge_requests
                    .iter_mut()
                    .filter(|mr| mr.state == QueueState::Pending)
                    .next()
                {
                    tracing::info!("Starting merge request {}", next_task.title);
                    next_task.state = QueueState::Running;
                }
            }
            if let Some(mr) = queue
                .merge_requests
                .iter_mut()
                .find(|mr| mr.state == QueueState::Running)
            {
                let details = get_mr_details(&gitlab_client, mr).await?;
                start_merge_progress(&gitlab_client, mr, &details).await?;
                check_merge_request_status(mr, &details).await?;
                check_pipeline_status(&gitlab_client, mr, &details).await?;
            }

            // TODO: print aggregated queue of all repositories
            queue.print()?;

            if queue
                .merge_requests
                .iter()
                .any(|mr| mr.state != QueueState::Running)
            {
                continue;
            }

            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }

    Ok(())
}
