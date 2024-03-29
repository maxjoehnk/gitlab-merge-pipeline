use crate::api::*;
use crate::merge_queue::{QueueEntry, QueueState};

pub async fn check_merge_request_status(
    entry: &mut QueueEntry,
    merge_request: &MergeRequestDetails,
) -> color_eyre::Result<()> {
    if !entry.state.is_running() {
        return Ok(());
    }
    if merge_request.has_conflicts {
        tracing::warn!("Aborting merge request {}: has conflicts", entry.title);
        entry.state = QueueState::Conflicts;
    }

    if merge_request.state == MergeRequestStatus::Merged {
        tracing::info!("Merge request {} merged", entry.title);
        entry.state = QueueState::Merged;
    }

    if merge_request.state == MergeRequestStatus::Closed {
        tracing::info!("Merge request {} was closed", entry.title);
        entry.state = QueueState::Closed;
    }

    Ok(())
}
