use gitlab::api::common::SortOrder;
use gitlab::api::projects::merge_requests::{MergeRequestOrderBy, MergeRequestState};
use gitlab::api::AsyncQuery;
use gitlab::AsyncGitlab;
use serde::Deserialize;

use crate::config::Repository;
use crate::merge_queue::{MergeQueue, QueueEntry, QueueState};

pub async fn fetch_merge_requests(
    gitlab_client: &AsyncGitlab,
    repository: &Repository,
    queue: &mut MergeQueue,
) -> color_eyre::Result<()> {
    let merge_requests = gitlab::api::projects::merge_requests::MergeRequests::builder()
        .project(&repository.name)
        .state(MergeRequestState::Opened)
        .labels(repository.labels.iter())
        .wip(false)
        .order_by(MergeRequestOrderBy::CreatedAt)
        .sort(SortOrder::Ascending)
        .build()?;
    let merge_requests = gitlab::api::paged(merge_requests, gitlab::api::Pagination::All);
    let merge_requests: Vec<MergeRequest> = merge_requests.query_async(gitlab_client).await?;

    for merge_request in merge_requests {
        if queue
            .merge_requests
            .iter()
            .any(|mr| mr.merge_request_id == merge_request.iid)
        {
            continue;
        }
        queue.merge_requests.push_back(QueueEntry {
            title: merge_request.title,
            merge_request_id: merge_request.iid,
            project_id: merge_request.project_id,
            state: QueueState::default(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct MergeRequest {
    title: String,
    iid: u64,
    project_id: u64,
}
