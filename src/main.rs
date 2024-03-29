use std::collections::VecDeque;
use std::time::Duration;
use gitlab::{api, AsyncGitlab, GitlabBuilder, Job, MergeRequest, MergeRequestInternalId, MergeStatus, ProjectId, StatusState};
use gitlab::api::{AsyncQuery};
use gitlab::api::common::SortOrder;
use gitlab::api::projects::merge_requests::{MergeRequestOrderBy, MergeRequestState};
use serde::Deserialize;
use tracing::level_filters::LevelFilter;
use yansi::Paint;
use crate::config::Repository;

mod args;
mod config;

#[derive(Default, Debug)]
struct MergeQueue {
    merge_requests: VecDeque<QueueEntry>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
enum QueueState {
    Merged,
    Running,
    #[default]
    Pending,
    RebaseFailed,
    PipelineFailed,
    Conflicts,
    Closed,
}

enum PipelineStatus {
    Running,
    Success,
    Waiting,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct QueueEntry {
    pub title: String,
    pub project_id: ProjectId,
    pub merge_request_id: MergeRequestInternalId,
    pub state: QueueState,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    setup_logging()?;

    let config = config::Config::read().await?;

    let gitlab_client = GitlabBuilder::new(config.gitlab.url.host().unwrap().to_string(), std::env::var("GITLAB_TOKEN")?)
        .build_async().await?;

    for repository in config.repositories {
        // TODO: spawn task per repository
        let mut queue = MergeQueue::default();

        loop {
            fetch_merge_requests(&gitlab_client, &repository, &mut queue).await?;
            if !queue.merge_requests.iter().any(|mr| mr.state == QueueState::Running) {
                if let Some(next_task) = queue.merge_requests.iter_mut().filter(|mr| mr.state == QueueState::Pending).next() {
                    start_merge_progress(&gitlab_client, next_task).await?;
                }
            }
            if let Some(mr) = queue.merge_requests.iter_mut().find(|mr| mr.state == QueueState::Running) {
                let details = get_mr_details(&gitlab_client, mr).await?;
                check_merge_request_status(mr, &details).await?;
                check_pipeline_status(&gitlab_client, mr, &details).await?;
            }

            // TODO: print aggregated queue of all repositories
            print_queue(&queue)?;
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }

    Ok(())
}

async fn fetch_merge_requests(gitlab_client: &AsyncGitlab, repository: &Repository, queue: &mut MergeQueue) -> color_eyre::Result<()> {
    let merge_requests = gitlab::api::projects::merge_requests::MergeRequests::builder()
        .project(&repository.name)
        .state(MergeRequestState::Opened)
        .labels(repository.labels.iter())
        .wip(false)
        .order_by(MergeRequestOrderBy::CreatedAt)
        .sort(SortOrder::Ascending)
        .build()?;
    let merge_requests = gitlab::api::paged(merge_requests, gitlab::api::Pagination::All);
    let merge_requests: Vec<MergeRequest> = merge_requests
        .query_async(gitlab_client)
        .await?;

    for merge_request in merge_requests {
        if queue.merge_requests.iter().any(|mr| mr.merge_request_id == merge_request.iid) {
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

fn setup_logging() -> color_eyre::Result<()> {
    let file_appender = tracing_appender::rolling::daily("logs", "merge-queue.log");
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .with_writer(file_appender)
        .init();

    Ok(())
}

async fn start_merge_progress(gitlab_client: &AsyncGitlab, entry: &mut QueueEntry) -> color_eyre::Result<()> {
    tracing::info!("Starting merge request {}", entry.title);
    let merge_request = get_mr_details(gitlab_client, entry).await?;

    if merge_request.needs_rebase() {
        if let Err(e) = rebase(gitlab_client, entry).await {
            entry.state = QueueState::RebaseFailed;
            return Err(e);
        }
    }

    if !merge_request.merge_when_pipeline_succeeds {
        if let Err(err) = enable_auto_merge(gitlab_client, entry).await {
            tracing::error!("Failed to enable auto merge for merge request {}: {}", entry.title, err);
        }
    }

    entry.state = QueueState::Running;

    Ok(())
}

async fn enable_auto_merge(client: &AsyncGitlab, entry: &QueueEntry) -> color_eyre::Result<()> {
    let enable_auto_merge = gitlab::api::projects::merge_requests::MergeMergeRequest::builder()
        .project(entry.project_id.value())
        .merge_request(entry.merge_request_id.value())
        .merge_when_pipeline_succeeds(true)
        .build()?;
    let enable_auto_merge = api::ignore(enable_auto_merge);
    enable_auto_merge.query_async(client).await?;

    Ok(())
}

async fn check_merge_request_status(entry: &mut QueueEntry, merge_request: &MergeRequestDetails) -> color_eyre::Result<()> {
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

async fn get_mr_details(gitlab_client: &AsyncGitlab, entry: &mut QueueEntry) -> color_eyre::Result<MergeRequestDetails> {
    let merge_request = gitlab::api::projects::merge_requests::MergeRequest::builder()
        .project(entry.project_id.value())
        .merge_request(entry.merge_request_id.value())
        .build()?;
    let merge_request: MergeRequestDetails = merge_request.query_async(gitlab_client).await?;
    tracing::debug!("{:#?}", merge_request);

    Ok(merge_request)
}

#[derive(Debug, Clone, Deserialize)]
struct MergeRequestDetails {
    pub has_conflicts: bool,
    pub draft: bool,
    pub merge_status: MergeStatus,
    pub state: MergeRequestStatus,
    pub detailed_merge_status: DetailedMergeStatus,
    pub head_pipeline: MergeRequestPipeline,
    pub merge_when_pipeline_succeeds: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct MergeRequestPipeline {
    pub id: u64,
    pub status: StatusState,
}

impl MergeRequestDetails {
    fn needs_rebase(&self) -> bool {
        self.detailed_merge_status == DetailedMergeStatus::NeedRebase
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetailedMergeStatus {
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

async fn rebase(gitlab_client: &AsyncGitlab, entry: &QueueEntry) -> color_eyre::Result<()> {
    tracing::info!("Rebasing merge request {}", entry.title);
    let rebase = gitlab::api::projects::merge_requests::RebaseMergeRequest::builder()
        .project(entry.project_id.value())
        .merge_request(entry.merge_request_id.value())
        .build()?;
    let rebase = api::ignore(rebase);
    rebase.query_async(gitlab_client).await?;

    Ok(())
}

async fn check_pipeline_status(gitlab_client: &AsyncGitlab, entry: &mut QueueEntry, details: &MergeRequestDetails) -> color_eyre::Result<()> {
    let jobs = gitlab::api::projects::pipelines::PipelineJobs::builder()
        .project(entry.project_id.value())
        .pipeline(details.head_pipeline.id)
        .scope(gitlab::api::projects::jobs::JobScope::Manual)
        .build()?;
    let jobs: Vec<Job> = jobs.query_async(gitlab_client).await?;
    let jobs: Vec<Job> = jobs.into_iter()
        .filter(|job| !job.allow_failure)
        .filter(|job| job.status == StatusState::Manual)
        .collect();
    tracing::debug!("{:#?}", &jobs);
    for required_job in jobs {
        tracing::info!("Triggering manual action for merge request {}", entry.title);
        let req = gitlab::api::projects::jobs::PlayJob::builder()
            .project(entry.project_id.value())
            .job(required_job.id.value())
            .build()?;
        let req = api::ignore(req);
        if let Err(err) = req.query_async(gitlab_client).await {
            tracing::error!("Failed to trigger manual action for merge request {}: {}", entry.title, err);
        }
    }
    match details.head_pipeline.status {
        StatusState::Failed => {
            tracing::warn!("Pipeline for merge request {} failed", entry.title);
            entry.state = QueueState::PipelineFailed;
        }
        _ => {}
    }

    Ok(())
}

fn print_queue(queue: &MergeQueue) -> color_eyre::Result<()> {
    print!("{}", termion::clear::All);
    let (columns, _) = termion::terminal_size()?;
    for entry in queue.merge_requests.iter() {
        let state = match entry.state {
            QueueState::Merged => "Merged".green(),
            QueueState::Closed => "Closed".yellow(),
            QueueState::Running => "Running".blue(),
            QueueState::Pending => "Pending".bright_black(),
            QueueState::RebaseFailed => "Rebase Failed".red(),
            QueueState::PipelineFailed => "Pipeline Failed".red(),
            QueueState::Conflicts => "Conflicts".red(),
        };
        let state = format!("[{state}]");
        let mut title = entry.title.clone();
        let required_space = title.len() + state.len();
        if required_space > columns as usize {
            let max_title = columns as usize - state.len() - 3;
            title.truncate(max_title);
        }
        let available_space = columns as usize - title.len() - state.len() + 3;
        let padding = " ".repeat(available_space);
        let title = if entry.state == QueueState::Running {
            title.bright_white()
        } else {
            title.white()
        };
        let state = state.white().wrap();

        println!("{title}{padding}{state}");
    }

    Ok(())
}
