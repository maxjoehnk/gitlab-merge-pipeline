use gitlab::api::AsyncQuery;
use gitlab::{AsyncGitlab, GitlabBuilder};
use serde::Deserialize;

use crate::config::Config;
use crate::merge_queue::QueueEntry;

pub async fn build_client(config: &Config) -> color_eyre::Result<AsyncGitlab> {
    let gitlab_client = GitlabBuilder::new(
        config.gitlab.url.host().unwrap().to_string(),
        &config.gitlab.token,
    )
    .build_async()
    .await?;
    Ok(gitlab_client)
}

pub async fn get_mr_details(
    gitlab_client: &AsyncGitlab,
    entry: &mut QueueEntry,
) -> color_eyre::Result<MergeRequestDetails> {
    let merge_request = gitlab::api::projects::merge_requests::MergeRequest::builder()
        .project(entry.project_id)
        .merge_request(entry.merge_request_id)
        .build()?;
    let merge_request: MergeRequestDetails = merge_request.query_async(gitlab_client).await?;
    tracing::debug!("{:#?}", merge_request);

    Ok(merge_request)
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeRequestDetails {
    pub has_conflicts: bool,
    pub state: MergeRequestStatus,
    pub detailed_merge_status: DetailedMergeStatus,
    pub head_pipeline: MergeRequestPipeline,
    pub merge_when_pipeline_succeeds: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct MergeRequestPipeline {
    pub id: u64,
    pub status: JobStatus,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetailedMergeStatus {
    BrokenStatus,
    BlockedStatus,
    Checking,
    Unchecked,
    CiMustPass,
    CiStillRunning,
    DiscussionsNotResolved,
    DraftStatus,
    ExternalStatusChecks,
    Mergeable,
    NotApproved,
    NotOpen,
    JiraAssociationMissing,
    NeedRebase,
    Conflict,
    RequestedChanges,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeRequestStatus {
    /// Filter merge requests that are open.
    Opened,
    /// Filter merge requests that are closed.
    Closed,
    /// Filter merge requests that are locked.
    Locked,
    /// Filter merge requests that are merged.
    Merged,
}

/// States for commit statuses.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// The check was created.
    Created,
    /// The check is waiting for some other resource.
    WaitingForResource,
    /// The check is currently being prepared.
    Preparing,
    /// The check is queued.
    Pending,
    /// The check is currently running.
    Running,
    /// The check succeeded.
    Success,
    /// The check failed.
    Failed,
    /// The check was canceled.
    Canceled,
    /// The check was skipped.
    Skipped,
    /// The check is waiting for manual action.
    Manual,
    /// The check is scheduled to run at some point in time.
    Scheduled,
}

/// Information about a job in Gitlab CI.
#[derive(Deserialize, Debug, Clone)]
pub struct Job {
    /// The ID of the job.
    pub id: u64,
    /// The status of the job.
    pub status: JobStatus,
    /// The name of the job.
    pub name: String,
    pub allow_failure: bool,
}
