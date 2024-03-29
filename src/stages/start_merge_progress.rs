use gitlab::api::AsyncQuery;
use gitlab::AsyncGitlab;

use crate::api::*;
use crate::merge_queue::{QueueEntry, QueueState};

pub async fn start_merge_progress(
    gitlab_client: &AsyncGitlab,
    entry: &mut QueueEntry,
    merge_request: &MergeRequestDetails,
) -> color_eyre::Result<()> {
    if merge_request.needs_rebase() {
        if let Err(e) = rebase(gitlab_client, entry).await {
            entry.state = QueueState::RebaseFailed;
            tracing::error!("Failed to rebase merge request {}: {}", entry.title, e);
            return Ok(());
        }
    }

    if !merge_request.merge_when_pipeline_succeeds {
        if let Err(err) = enable_auto_merge(gitlab_client, entry).await {
            tracing::error!(
                "Failed to enable auto merge for merge request {}: {}",
                entry.title,
                err
            );
        }
    }

    Ok(())
}

async fn enable_auto_merge(client: &AsyncGitlab, entry: &QueueEntry) -> color_eyre::Result<()> {
    let enable_auto_merge = gitlab::api::projects::merge_requests::MergeMergeRequest::builder()
        .project(entry.project_id)
        .merge_request(entry.merge_request_id)
        .merge_when_pipeline_succeeds(true)
        .build()?;
    let enable_auto_merge = gitlab::api::ignore(enable_auto_merge);
    enable_auto_merge.query_async(client).await?;

    Ok(())
}

async fn rebase(gitlab_client: &AsyncGitlab, entry: &QueueEntry) -> color_eyre::Result<()> {
    tracing::info!("Rebasing merge request {}", entry.title);
    let rebase = gitlab::api::projects::merge_requests::RebaseMergeRequest::builder()
        .project(entry.project_id)
        .merge_request(entry.merge_request_id)
        .build()?;
    let rebase = gitlab::api::ignore(rebase);
    rebase.query_async(gitlab_client).await?;

    Ok(())
}

impl MergeRequestDetails {
    fn needs_rebase(&self) -> bool {
        self.detailed_merge_status == DetailedMergeStatus::NeedRebase
    }
}
