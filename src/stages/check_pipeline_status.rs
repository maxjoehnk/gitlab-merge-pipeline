use crate::api::*;
use crate::merge_queue::{QueueEntry, QueueState};

pub async fn check_pipeline_status(
    entry: &mut QueueEntry,
    details: &MergeRequestDetails,
) -> color_eyre::Result<()> {
    if !entry.is_running() {
        return Ok(());
    }
    if details.head_pipeline.status == JobStatus::Failed {
        tracing::warn!("Pipeline for merge request {} failed", entry.title);
        entry.change_state(QueueState::PipelineFailed);
    }
    
    Ok(())
}

pub async fn check_failed_jobs(
    api_client: &ApiClient,
    entry: &mut QueueEntry,
    details: &MergeRequestDetails,
) -> color_eyre::Result<()> {
    if !entry.is_running() {
        return Ok(());
    }
    let jobs = api_client.get_jobs(entry, details.head_pipeline.id).await?;

    let failed_jobs = jobs
        .into_iter()
        .filter(|job| !job.allow_failure)
        .filter(|job| matches!(job.status, JobStatus::Failed | JobStatus::Canceled))
        .collect::<Vec<_>>();

    if !failed_jobs.is_empty() {
        tracing::info!("Failed jobs: {:#?}", failed_jobs);
        tracing::warn!("Pipeline for merge request {} failed", entry.title);
        entry.change_state(QueueState::PipelineFailed);
    }

    Ok(())
}

pub async fn start_manual_jobs(
    api_client: &ApiClient,
    entry: &mut QueueEntry,
    details: &MergeRequestDetails,
) -> color_eyre::Result<()> {
    if !entry.is_running() {
        return Ok(());
    }
    let jobs = api_client.get_jobs(entry, details.head_pipeline.id).await?;

    let manual_jobs = jobs
        .into_iter()
        .filter(|job| !job.allow_failure)
        .filter(|job| matches!(job.status, JobStatus::Manual))
        .collect::<Vec<_>>();

    for required_job in manual_jobs {
        tracing::info!("Triggering manual action for merge request {}", entry.title);
        if let Err(err) = api_client.run_job(entry, &required_job).await {
            tracing::error!(
                "Failed to trigger manual action for merge request {}: {}",
                entry.title,
                err
            );
        }
    }

    Ok(())
}