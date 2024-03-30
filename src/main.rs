use std::time::Duration;

use tokio::task::JoinHandle;

use crate::aggregator::spawn_aggregator;
use crate::api::*;
use crate::logging::setup_logging;
use crate::stages::*;

use self::merge_queue::*;

mod api;
mod config;
mod logging;
mod merge_queue;
mod stages;
mod aggregator;

#[tokio::main(flavor = "current_thread")]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let _guard = setup_logging()?;

    let config = config::Config::read().await?;

    let mut task_handles = Vec::with_capacity(config.repositories.len());

    let queue_sender = spawn_aggregator();

    for repository in config.repositories {
        let gitlab_config = config.gitlab.clone();
        let queue_sender = queue_sender.clone();
        let handle: JoinHandle<color_eyre::Result<()>> = tokio::task::spawn(async move {
            let api_client = ApiClient::new(&gitlab_config).await?;

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
                        .find(|mr| mr.is_pending())
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

                queue_sender.send((repository.name.clone(), queue.clone()))?;

                if queue
                    .merge_requests
                    .iter()
                    .any(|mr| !mr.is_running())
                {
                    continue;
                }

                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });

        task_handles.push(handle);
    }

    for handle in task_handles {
        handle.await??;
    }

    Ok(())
}
