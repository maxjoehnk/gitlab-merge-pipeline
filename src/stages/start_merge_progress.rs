use crate::api::*;
use crate::merge_queue::{QueueEntry, QueueState};

pub async fn rebase_when_necessary(
    api_client: &ApiClient,
    entry: &mut QueueEntry,
    merge_request: &MergeRequestDetails,
) -> color_eyre::Result<bool> {
    if merge_request.rebase_in_progress {
        return Ok(true);
    }
    if merge_request.needs_rebase() {
        tracing::info!("Rebasing merge request {}", entry.title);
        if let Err(e) = api_client.rebase(entry).await {
            entry.change_state(QueueState::RebaseFailed);
            tracing::error!("Failed to rebase merge request {}: {}", entry.title, e);
        }
        return Ok(true);
    }

    Ok(false)
}

pub async fn ensure_automerge(
    api_client: &ApiClient,
    entry: &mut QueueEntry,
    merge_request: &MergeRequestDetails,
) -> color_eyre::Result<()> {
    if !entry.is_running() {
        return Ok(());
    }

    if !merge_request.merge_when_pipeline_succeeds {
        if let Err(err) = api_client.enable_auto_merge(entry).await {
            tracing::error!(
                "Failed to enable auto merge for merge request {}: {}",
                entry.title,
                err
            );
        }
    }

    Ok(())
}
