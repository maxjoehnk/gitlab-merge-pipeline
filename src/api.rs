use chrono::{DateTime, Utc};
use gitlab::api::AsyncQuery;
use gitlab::{AsyncGitlab, GitlabBuilder};
use gitlab::api::common::SortOrder;
use gitlab::api::projects::merge_requests::{MergeRequestOrderBy, MergeRequestState};
use serde::Deserialize;

use crate::config::{GitlabConfig, Repository};
use crate::merge_queue::QueueEntry;

pub struct ApiClient {
    gitlab_client: AsyncGitlab,
}

impl ApiClient {
    pub async fn new(config: &GitlabConfig) -> color_eyre::Result<Self> {
        let gitlab_client = build_client(config).await?;

        Ok(Self { gitlab_client })
    }

    pub async fn get_details(&self, entry: &QueueEntry) -> color_eyre::Result<MergeRequestDetails> {
        let merge_request = gitlab::api::projects::merge_requests::MergeRequest::builder()
            .project(entry.project_id)
            .merge_request(entry.merge_request_id)
            .build()?;
        let merge_request: MergeRequestDetails = merge_request.query_async(&self.gitlab_client).await?;
        tracing::debug!("{:#?}", merge_request);

        Ok(merge_request)
    }

    pub async fn fetch_merge_requests(&self, repository: &Repository) -> color_eyre::Result<Vec<MergeRequest>> {
        let merge_requests = gitlab::api::projects::merge_requests::MergeRequests::builder()
            .project(&repository.name)
            .state(MergeRequestState::Opened)
            .labels(repository.labels.iter())
            .wip(false)
            .order_by(MergeRequestOrderBy::CreatedAt)
            .sort(SortOrder::Ascending)
            .build()?;
        let merge_requests = gitlab::api::paged(merge_requests, gitlab::api::Pagination::All);
        let merge_requests: Vec<MergeRequest> = merge_requests.query_async(&self.gitlab_client).await?;

        Ok(merge_requests)
    }

    pub async fn get_jobs(&self, entry: &QueueEntry, pipeline_id: u64) -> color_eyre::Result<Vec<Job>> {
        let jobs = gitlab::api::projects::pipelines::PipelineJobs::builder()
            .project(entry.project_id)
            .pipeline(pipeline_id)
            .build()?;
        let jobs = gitlab::api::paged(jobs, gitlab::api::Pagination::All);
        let jobs: Vec<Job> = jobs.query_async(&self.gitlab_client).await?;
        tracing::trace!("{:#?}", &jobs);

        Ok(jobs)
    }

    pub async fn run_job(&self, entry: &QueueEntry, job: &Job) -> color_eyre::Result<()> {
        let req = gitlab::api::projects::jobs::PlayJob::builder()
            .project(entry.project_id)
            .job(job.id)
            .build()?;
        let req = gitlab::api::ignore(req);
        req.query_async(&self.gitlab_client).await?;

        Ok(())
    }

    pub async fn enable_auto_merge(&self, entry: &QueueEntry) -> color_eyre::Result<()> {
        let enable_auto_merge = gitlab::api::projects::merge_requests::MergeMergeRequest::builder()
            .project(entry.project_id)
            .merge_request(entry.merge_request_id)
            .merge_when_pipeline_succeeds(true)
            .build()?;
        let enable_auto_merge = gitlab::api::ignore(enable_auto_merge);
        enable_auto_merge.query_async(&self.gitlab_client).await?;

        Ok(())
    }

    pub async fn rebase(&self, entry: &QueueEntry) -> color_eyre::Result<()> {
        let rebase = gitlab::api::projects::merge_requests::RebaseMergeRequest::builder()
            .project(entry.project_id)
            .merge_request(entry.merge_request_id)
            .build()?;
        let rebase = gitlab::api::ignore(rebase);
        rebase.query_async(&self.gitlab_client).await?;

        Ok(())
    }
}

async fn build_client(config: &GitlabConfig) -> color_eyre::Result<AsyncGitlab> {
    let gitlab_client = GitlabBuilder::new(
        config.url.host().unwrap().to_string(),
        &config.token,
    )
    .build_async()
    .await?;

    Ok(gitlab_client)
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

#[derive(Debug, Clone, Deserialize)]
pub struct MergeRequest {
    pub title: String,
    pub iid: u64,
    pub project_id: u64,
    pub updated_at: DateTime<Utc>,
}
