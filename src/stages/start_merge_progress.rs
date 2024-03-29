use crate::api::*;
use crate::merge_queue::{QueueEntry, QueueState};

pub async fn start_merge_progress(
    api_client: &ApiClient,
    entry: &mut QueueEntry,
    merge_request: &MergeRequestDetails,
) -> color_eyre::Result<()> {
    if merge_request.needs_rebase() {
        tracing::info!("Rebasing merge request {}", entry.title);
        if let Err(e) = api_client.rebase(entry).await {
            entry.change_state(QueueState::RebaseFailed);
            tracing::error!("Failed to rebase merge request {}: {}", entry.title, e);
            return Ok(());
        }
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


impl MergeRequestDetails {
    fn needs_rebase(&self) -> bool {
        self.detailed_merge_status == DetailedMergeStatus::NeedRebase
    }
}
