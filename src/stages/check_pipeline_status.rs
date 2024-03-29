use gitlab::api::AsyncQuery;
use gitlab::AsyncGitlab;

use crate::api::*;
use crate::merge_queue::{QueueEntry, QueueState};

pub async fn check_pipeline_status(
    gitlab_client: &AsyncGitlab,
    entry: &mut QueueEntry,
    details: &MergeRequestDetails,
) -> color_eyre::Result<()> {
    if !entry.state.is_running() {
        return Ok(());
    }
    if details.head_pipeline.status == JobStatus::Failed {
        tracing::warn!("Pipeline for merge request {} failed", entry.title);
        entry.state = QueueState::PipelineFailed;

        return Ok(());
    }

    let jobs = gitlab::api::projects::pipelines::PipelineJobs::builder()
        .project(entry.project_id)
        .pipeline(details.head_pipeline.id)
        .build()?;
    let jobs = gitlab::api::paged(jobs, gitlab::api::Pagination::All);
    let jobs: Vec<Job> = jobs.query_async(gitlab_client).await?;
    tracing::trace!("{:#?}", &jobs);
    let (manual_jobs, jobs) = jobs
        .into_iter()
        .filter(|job| !job.allow_failure)
        .partition::<Vec<_>, _>(|job| job.status == JobStatus::Manual);

    let has_failed_jobs = jobs
        .into_iter()
        .filter(|job| matches!(job.status, JobStatus::Failed | JobStatus::Canceled))
        .count()
        > 0;

    if has_failed_jobs {
        tracing::warn!("Pipeline for merge request {} failed", entry.title);
        entry.state = QueueState::PipelineFailed;

        return Ok(());
    }

    let manual_jobs: Vec<Job> = manual_jobs.into_iter().collect();
    for required_job in manual_jobs {
        tracing::info!("Triggering manual action for merge request {}", entry.title);
        let req = gitlab::api::projects::jobs::PlayJob::builder()
            .project(entry.project_id)
            .job(required_job.id)
            .build()?;
        let req = gitlab::api::ignore(req);
        if let Err(err) = req.query_async(gitlab_client).await {
            tracing::error!(
                "Failed to trigger manual action for merge request {}: {}",
                entry.title,
                err
            );
        }
    }

    Ok(())
}
