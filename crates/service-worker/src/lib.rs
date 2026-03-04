use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_auth::AuthService;
use core_storage::StorageLayer;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("queue error: {0}")]
    Queue(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerTask {
    RebuildMailboxIndex { mailbox_name: String },
    ExpireSessionTokens { limit: usize },
    CleanupDeletedMessages { batch_size: usize },
}

#[async_trait]
pub trait WorkerQueue: Send + Sync {
    async fn dequeue_batch(&self, batch_size: usize) -> Result<Vec<WorkerTask>, WorkerError>;
}

#[derive(Default)]
pub struct MockQueue {
    tasks: Mutex<VecDeque<WorkerTask>>,
}

impl MockQueue {
    pub fn with_seed_tasks(tasks: Vec<WorkerTask>) -> Self {
        Self {
            tasks: Mutex::new(VecDeque::from(tasks)),
        }
    }
}

#[async_trait]
impl WorkerQueue for MockQueue {
    async fn dequeue_batch(&self, batch_size: usize) -> Result<Vec<WorkerTask>, WorkerError> {
        let mut tasks = self.tasks.lock().await;
        let mut batch = Vec::new();

        for _ in 0..batch_size {
            let Some(task) = tasks.pop_front() else {
                break;
            };
            batch.push(task);
        }

        Ok(batch)
    }
}

pub struct Worker {
    storage: Arc<StorageLayer>,
    auth: Arc<AuthService>,
    queue: Arc<dyn WorkerQueue>,
    poll_interval: Duration,
    batch_size: usize,
}

impl Worker {
    pub fn new(storage: Arc<StorageLayer>, auth: Arc<AuthService>) -> Self {
        let queue = Arc::new(MockQueue::with_seed_tasks(vec![
            WorkerTask::RebuildMailboxIndex {
                mailbox_name: "INBOX".to_string(),
            },
            WorkerTask::ExpireSessionTokens { limit: 1000 },
            WorkerTask::CleanupDeletedMessages { batch_size: 500 },
        ]));

        Self {
            storage,
            auth,
            queue,
            poll_interval: Duration::from_secs(2),
            batch_size: 16,
        }
    }

    pub fn with_queue(mut self, queue: Arc<dyn WorkerQueue>) -> Self {
        self.queue = queue;
        self
    }

    pub fn with_poll_interval(mut self, poll_interval: Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.max(1);
        self
    }

    pub fn start(self) -> JoinHandle<Result<(), WorkerError>> {
        tokio::spawn(async move { self.run().await })
    }

    pub async fn run(self) -> Result<(), WorkerError> {
        let mut interval = tokio::time::interval(self.poll_interval);
        info!("worker started");

        loop {
            interval.tick().await;
            self.run_once().await?;
        }
    }

    pub async fn run_once(&self) -> Result<(), WorkerError> {
        let tasks = self.queue.dequeue_batch(self.batch_size).await?;
        if tasks.is_empty() {
            debug!("worker queue is empty");
            return Ok(());
        }

        for task in tasks {
            let task_json = serde_json::to_string(&task)?;
            let parsed_task: WorkerTask = serde_json::from_str(&task_json)?;
            self.process_task(parsed_task).await?;
        }

        Ok(())
    }

    async fn process_task(&self, task: WorkerTask) -> Result<(), WorkerError> {
        let _ = (&self.storage, &self.auth);

        match task {
            WorkerTask::RebuildMailboxIndex { mailbox_name } => {
                info!(mailbox = %mailbox_name, "worker rebuilding mailbox index");
            }
            WorkerTask::ExpireSessionTokens { limit } => {
                info!(limit, "worker expiring session tokens");
            }
            WorkerTask::CleanupDeletedMessages { batch_size } => {
                info!(batch_size, "worker cleaning deleted messages");
            }
        }

        Ok(())
    }
}

pub async fn run_worker(worker: Worker) -> Result<(), WorkerError> {
    warn!("run_worker starts an endless processing loop");
    worker.run().await
}

pub mod worker {
    pub use super::{run_worker, MockQueue, Worker, WorkerError, WorkerQueue, WorkerTask};
}
