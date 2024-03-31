use std::time::Duration;
use crate::api::*;
use crate::merge_queue::{QueueEntry, QueueState};

pub async fn check_merge_request_status(
    entry: &mut QueueEntry,
    merge_request: &MergeRequestDetails,
) -> color_eyre::Result<()> {
    if !entry.is_running() {
        return Ok(());
    }
    if merge_request.has_conflicts {
        tracing::warn!("Aborting merge request {}: has conflicts", entry.title);
        entry.change_state(QueueState::Conflicts);
    }

    if merge_request.state == MergeRequestStatus::Merged {
        tracing::info!("Merge request {} merged", entry.title);
        entry.change_state(QueueState::Merged);
        // Wait until gitlab has processed the changes in main
        tokio::time::sleep(Duration::from_secs(60)).await;
    }

    if merge_request.state == MergeRequestStatus::Closed {
        tracing::info!("Merge request {} was closed", entry.title);
        entry.change_state(QueueState::Closed);
    }

    Ok(())
}
