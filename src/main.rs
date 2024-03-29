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

    let api_client = ApiClient::new(&config).await?;

    for repository in config.repositories {
        // TODO: spawn task per repository
        let mut queue = MergeQueue::default();

        loop {
            fetch_merge_requests(&api_client, &repository, &mut queue).await?;
            if !queue
                .merge_requests
                .iter()
                .any(|mr| mr.is_running())
            {
                if let Some(next_task) = queue
                    .merge_requests
                    .iter_mut()
                    .filter(|mr| mr.is_pending())
                    .next()
                {
                    tracing::info!("Starting merge request {}", next_task.title);
                    next_task.change_state(QueueState::Running);
                }
            }
            if let Some(mr) = queue
                .merge_requests
                .iter_mut()
                .find(|mr| mr.is_running())
            {
                let details = api_client.get_details(mr).await?;
                start_merge_progress(&api_client, mr, &details).await?;
                check_merge_request_status(mr, &details).await?;
                check_pipeline_status(&api_client, mr, &details).await?;
            }

            // TODO: print aggregated queue of all repositories
            queue.print()?;

            if queue
                .merge_requests
                .iter()
                .any(|mr| !mr.is_running())
            {
                continue;
            }

            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }

    Ok(())
}
