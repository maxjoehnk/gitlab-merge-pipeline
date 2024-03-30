use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use yansi::Paint;

use crate::api::MergeRequest;

#[derive(Default, Debug, Clone)]
pub struct MergeQueue {
    pub merge_requests: VecDeque<QueueEntry>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum QueueState {
    Merged,
    Running,
    #[default]
    Pending,
    RebaseFailed,
    PipelineFailed,
    Conflicts,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct QueueEntry {
    pub title: String,
    pub project_id: u64,
    pub merge_request_id: u64,
    state: QueueState,
    last_state_change: DateTime<Utc>,
}

impl QueueEntry {
    pub fn is_running(&self) -> bool {
        matches!(self.state, QueueState::Running)
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.state, QueueState::Pending)
    }

    pub fn change_state(&mut self, state: QueueState) {
        self.state = state;
        self.last_state_change = Utc::now();
    }
    
    fn should_update(&self, merge_request: &MergeRequest) -> bool {
        self.last_state_change < merge_request.updated_at
    }
}

impl From<MergeRequest> for QueueEntry {
    fn from(merge_request: MergeRequest) -> Self {
        Self {
            title: merge_request.title,
            merge_request_id: merge_request.iid,
            project_id: merge_request.project_id,
            state: QueueState::default(),
            last_state_change: Utc::now(),
        }
    }
}

impl MergeQueue {
    pub fn print(&self) -> color_eyre::Result<()> {
        let (columns, _) = termion::terminal_size()?;
        for entry in self.merge_requests.iter() {
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

    pub fn try_add_merge_request(&mut self, merge_request: MergeRequest) {
        if self.merge_requests
            .iter()
            .any(|mr| mr.merge_request_id == merge_request.iid && !mr.should_update(&merge_request))
        {
            return;
        }
        if let Some(index) = self.merge_requests
            .iter()
            .position(|mr| mr.merge_request_id == merge_request.iid)
        {
            self.merge_requests.remove(index);
        }
        self.merge_requests.push_back(QueueEntry::from(merge_request));
    }
}
