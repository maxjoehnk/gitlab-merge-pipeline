use std::collections::VecDeque;
use yansi::Paint;

#[derive(Default, Debug)]
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

impl QueueState {
    pub fn is_running(&self) -> bool {
        matches!(self, QueueState::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct QueueEntry {
    pub title: String,
    pub project_id: u64,
    pub merge_request_id: u64,
    pub state: QueueState,
}

impl MergeQueue {
    pub fn print(&self) -> color_eyre::Result<()> {
        print!("{}", termion::clear::All);
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
}
