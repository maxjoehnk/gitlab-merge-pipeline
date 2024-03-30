use std::collections::HashMap;

use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use yansi::Paint;

use crate::merge_queue::MergeQueue;

pub fn spawn_aggregator() -> UnboundedSender<(String, MergeQueue)> {
    let (queue_sender, queue_receiver) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut stream = UnboundedReceiverStream::new(queue_receiver);
        let mut repositories = HashMap::<String, MergeQueue>::new();
        while let Some((repository, queue)) = stream.next().await {
            repositories.insert(repository, queue);

            println!("{}", termion::clear::All);
            for (repository, queue) in &repositories {
                println!("{}", repository.bold());
                if let Err(err) = queue.print() {
                    tracing::error!("Failed to print queue: {}", err);
                }
            }
        }
    });

    queue_sender
}