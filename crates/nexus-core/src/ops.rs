//! Asynchronous operation registry, cancellation tokens, and bounded workers.

use std::collections::{BTreeMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::error::CoreError;

/// Number of worker threads used for bounded asynchronous operations.
pub const OPERATION_WORKERS: usize = 4;
const RETAINED_TERMINAL_OPERATIONS: usize = 4_096;

/// Cancellation state shared with a running operation.
pub struct CancelToken {
    cancelled: Mutex<bool>,
    changed: Condvar,
}

impl CancelToken {
    /// Creates a token in the running state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancelled: Mutex::new(false),
            changed: Condvar::new(),
        }
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        let mut cancelled = self.cancelled.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *cancelled = true;
        self.changed.notify_all();
    }

    /// Whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Waits for the duration or until cancellation. Returns true when
    /// cancellation interrupted the wait.
    pub fn wait(&self, duration: Duration) -> bool {
        if duration.is_zero() {
            return self.is_cancelled();
        }
        let cancelled = self.cancelled.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let (cancelled, _) = self
            .changed
            .wait_timeout_while(cancelled, duration, |cancelled| !*cancelled)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *cancelled
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CancelToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CancelToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

/// Result of a cancellation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelOutcome {
    /// The operation was running and is now cancelled.
    Requested,
    /// The operation had already reached a terminal event.
    AlreadyTerminal,
    /// The operation ID is unknown.
    Unknown,
}

struct OperationEntry {
    token: Arc<CancelToken>,
    terminal: bool,
}

struct RegistryState {
    entries: BTreeMap<u64, OperationEntry>,
    terminal_order: VecDeque<u64>,
}

/// Tracks running and recently terminal asynchronous operations.
pub struct OperationRegistry {
    next_id: AtomicU64,
    state: Mutex<RegistryState>,
}

impl OperationRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            state: Mutex::new(RegistryState {
                entries: BTreeMap::new(),
                terminal_order: VecDeque::new(),
            }),
        }
    }

    /// Allocates an operation ID and cancellation token.
    pub fn begin(&self) -> (u64, Arc<CancelToken>) {
        let op_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let token = Arc::new(CancelToken::new());
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.entries.insert(
            op_id,
            OperationEntry {
                token: Arc::clone(&token),
                terminal: false,
            },
        );
        (op_id, token)
    }

    /// Marks an operation terminal.
    pub fn finish(&self, op_id: u64) {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(entry) = state.entries.get_mut(&op_id)
            && !entry.terminal
        {
            entry.terminal = true;
            state.terminal_order.push_back(op_id);
        }
        while state.terminal_order.len() > RETAINED_TERMINAL_OPERATIONS {
            if let Some(old) = state.terminal_order.pop_front() {
                state.entries.remove(&old);
            }
        }
    }

    /// Requests cancellation of one operation.
    pub fn cancel(&self, op_id: u64) -> CancelOutcome {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match state.entries.get(&op_id) {
            None => CancelOutcome::Unknown,
            Some(entry) if entry.terminal => CancelOutcome::AlreadyTerminal,
            Some(entry) => {
                entry.token.cancel();
                CancelOutcome::Requested
            }
        }
    }

    /// Cancels every running operation.
    pub fn cancel_all(&self) {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for entry in state.entries.values().filter(|entry| !entry.terminal) {
            entry.token.cancel();
        }
    }

    /// Number of running operations.
    #[must_use]
    pub fn running_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entries
            .values()
            .filter(|entry| !entry.terminal)
            .count()
    }
}

impl Default for OperationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Fixed-size worker pool with no unbounded per-operation thread creation.
pub struct WorkerPool {
    sender: Option<Sender<Job>>,
    workers: Vec<JoinHandle<()>>,
}

impl WorkerPool {
    /// Starts the pool.
    pub fn new(name: &'static str, workers: usize) -> Result<Self, CoreError> {
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut handles = Vec::with_capacity(workers);
        for index in 0..workers {
            let receiver = Arc::clone(&receiver);
            let handle = thread::Builder::new()
                .name(format!("{name}-{index}"))
                .spawn(move || {
                    loop {
                        let job = {
                            let receiver = receiver.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                            receiver.recv()
                        };
                        match job {
                            Ok(job) => {
                                let _ = catch_unwind(AssertUnwindSafe(job));
                            }
                            Err(_) => break,
                        }
                    }
                })
                .map_err(|error| CoreError::Internal(format!("worker thread failed to start: {error}")))?;
            handles.push(handle);
        }
        Ok(Self {
            sender: Some(sender),
            workers: handles,
        })
    }

    /// Submits a job to the pool.
    pub fn submit(&self, job: impl FnOnce() + Send + 'static) -> Result<(), CoreError> {
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(|| CoreError::Internal("worker pool is shutting down".to_owned()))?;
        sender
            .send(Box::new(job))
            .map_err(|_| CoreError::Internal("worker pool is shutting down".to_owned()))
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.sender.take();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::{CancelOutcome, CancelToken, OperationRegistry, WorkerPool};

    #[test]
    fn cancellation_is_idempotent_and_observable() {
        let registry = OperationRegistry::new();
        let (op_id, _token) = registry.begin();
        assert_eq!(registry.running_count(), 1);
        assert_eq!(registry.cancel(op_id), CancelOutcome::Requested);
        assert_eq!(registry.cancel(op_id), CancelOutcome::Requested);
        registry.finish(op_id);
        assert_eq!(registry.cancel(op_id), CancelOutcome::AlreadyTerminal);
        assert_eq!(registry.cancel(op_id + 1), CancelOutcome::Unknown);
        assert_eq!(registry.running_count(), 0);
    }

    #[test]
    fn cancel_token_interrupts_a_wait() {
        let token = CancelToken::new();
        let clone = std::sync::Arc::new(token);
        let waiter = {
            let clone = std::sync::Arc::clone(&clone);
            std::thread::spawn(move || clone.wait(Duration::from_secs(30)))
        };
        std::thread::sleep(Duration::from_millis(10));
        clone.cancel();
        assert!(waiter.join().expect("waiter"));
        assert!(clone.is_cancelled());
    }

    #[test]
    fn worker_pool_runs_bounded_jobs() {
        let pool = WorkerPool::new("test-pool", 2).expect("pool");
        let (sender, receiver) = mpsc::channel();
        for value in 0..8 {
            let sender = sender.clone();
            pool.submit(move || sender.send(value).expect("send")).expect("submit");
        }
        drop(sender);
        let values: Vec<u64> = receiver.iter().collect();
        assert_eq!(values.len(), 8);
    }
}
