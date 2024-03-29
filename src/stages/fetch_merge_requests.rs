use crate::api::ApiClient;

use crate::config::Repository;
use crate::merge_queue::{MergeQueue};

pub async fn fetch_merge_requests(
    api_client: &ApiClient,
    repository: &Repository,
    queue: &mut MergeQueue,
) -> color_eyre::Result<()> {
    let merge_requests = api_client.fetch_merge_requests(repository).await?;

    for merge_request in merge_requests {
        queue.try_add_merge_request(merge_request);
    }
    Ok(())
}
