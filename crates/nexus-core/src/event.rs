//! Bounded, coalescing event queue owned by the core.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nexus_crypto::Value;

use crate::envelope::{EnvelopeError, MAX_EVENT_BYTES, encode_event};

/// Maximum queued events.
pub const EVENT_QUEUE_CAPACITY: usize = 1_024;

/// Priority class for an event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventClass {
    /// Best-effort progress that may be coalesced under backpressure.
    Progress,
    /// State or terminal event that is never dropped.
    Durable,
}

/// An event before queue assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingEvent {
    /// Stable event type such as `core.state` or `test.progress`.
    pub event_type: String,
    /// Asynchronous operation ID when applicable.
    pub op_id: Option<u64>,
    /// Download ID when applicable.
    pub download_id: Option<String>,
    /// Bounded data map.
    pub data: Value,
    /// Queue priority.
    pub class: EventClass,
}

impl PendingEvent {
    /// Creates a durable state or terminal event.
    #[must_use]
    pub fn durable(event_type: impl Into<String>, data: Value) -> Self {
        Self {
            event_type: event_type.into(),
            op_id: None,
            download_id: None,
            data,
            class: EventClass::Durable,
        }
    }

    /// Creates a coalescible progress event.
    #[must_use]
    pub fn progress(event_type: impl Into<String>, data: Value) -> Self {
        Self {
            event_type: event_type.into(),
            op_id: None,
            download_id: None,
            data,
            class: EventClass::Progress,
        }
    }

    /// Attaches an operation ID.
    #[must_use]
    pub fn with_op(mut self, op_id: u64) -> Self {
        self.op_id = Some(op_id);
        self
    }

    /// Attaches a download ID.
    #[must_use]
    pub fn with_download(mut self, download_id: impl Into<String>) -> Self {
        self.download_id = Some(download_id.into());
        self
    }
}

/// A queued event with its assigned sequence and timestamp.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    /// Monotonic queue sequence number.
    pub seq: u64,
    /// Unix milliseconds when the event was enqueued.
    pub timestamp_ms: u64,
    /// Stable event type.
    pub event_type: String,
    /// Asynchronous operation ID when applicable.
    pub op_id: Option<u64>,
    /// Download ID when applicable.
    pub download_id: Option<String>,
    /// Bounded data map.
    pub data: Value,
    /// Queue priority copied from the pending event.
    pub class: EventClass,
}

impl Event {
    /// Encodes the event envelope.
    pub fn encode(&self) -> Result<Vec<u8>, EventError> {
        Ok(encode_event(self)?)
    }

    /// Returns the encoded size.
    pub fn encoded_len(&self) -> Result<usize, EventError> {
        Ok(self.encode()?.len())
    }
}

/// Event queue failure.
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    /// The event was larger than the ABI limit.
    #[error("event is {actual} bytes, above the {max}-byte limit")]
    TooLarge {
        /// Observed size.
        actual: usize,
        /// Maximum size.
        max: usize,
    },
    /// The queue was closed.
    #[error("the event queue is closed")]
    Closed,
    /// The queue is full of durable events and a non-blocking push was refused.
    #[error("the event queue is full of durable events")]
    Full,
    /// Envelope encoding failed.
    #[error(transparent)]
    Envelope(#[from] EnvelopeError),
}

struct QueueState {
    events: VecDeque<Event>,
    closed: bool,
    next_seq: u64,
}

impl QueueState {
    fn push_back(&mut self, event: Event) {
        self.events.push_back(event);
    }

    fn remove_progress_for(&mut self, op_id: Option<u64>, event_type: &str) {
        if let Some(index) = self.events.iter().position(|event| {
            event.class == EventClass::Progress && event.op_id == op_id && event.event_type == event_type
        }) {
            self.events.remove(index);
        }
    }

    fn remove_oldest_progress(&mut self) -> bool {
        if let Some(index) = self.events.iter().position(|event| event.class == EventClass::Progress) {
            self.events.remove(index);
            return true;
        }
        false
    }
}

/// Thread-safe bounded event queue with one blocking consumer.
pub struct EventQueue {
    state: Mutex<QueueState>,
    available: Condvar,
}

impl EventQueue {
    /// Creates an empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(QueueState {
                events: VecDeque::with_capacity(EVENT_QUEUE_CAPACITY),
                closed: false,
                next_seq: 1,
            }),
            available: Condvar::new(),
        }
    }

    /// Enqueues a durable event, waiting for capacity when necessary.
    pub fn push(&self, event: PendingEvent) -> Result<u64, EventError> {
        if event.class == EventClass::Progress {
            return self.push_progress(event);
        }
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            if state.closed {
                return Err(EventError::Closed);
            }
            if state.events.len() < EVENT_QUEUE_CAPACITY {
                break;
            }
            if state.remove_oldest_progress() {
                break;
            }
            state = self
                .available
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        let event = make_event(&mut state, event)?;
        state.push_back(event.clone());
        Ok(event.seq)
    }

    fn push_progress(&self, pending: PendingEvent) -> Result<u64, EventError> {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.closed {
            return Err(EventError::Closed);
        }
        // Keep at most one pending progress event per (operation, type).
        state.remove_progress_for(pending.op_id, &pending.event_type);
        if state.events.len() >= EVENT_QUEUE_CAPACITY && !state.remove_oldest_progress() {
            // Progress is best-effort; a full queue of durable events drops it.
            return Ok(0);
        }
        let event = make_event(&mut state, pending)?;
        state.push_back(event.clone());
        Ok(event.seq)
    }

    /// Enqueues a durable event without waiting for capacity.
    ///
    /// Lifecycle transitions use this path so a stopped or absent event pump
    /// can never deadlock `start` or `stop`. Progress is still coalesced and
    /// the oldest progress event is evicted first. If the queue contains only
    /// durable events, the new event is refused with [`EventError::Full`].
    pub fn try_push(&self, event: PendingEvent) -> Result<u64, EventError> {
        if event.class == EventClass::Progress {
            return self.push_progress(event);
        }
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.closed {
            return Err(EventError::Closed);
        }
        if state.events.len() >= EVENT_QUEUE_CAPACITY && !state.remove_oldest_progress() {
            return Err(EventError::Full);
        }
        let event = make_event(&mut state, event)?;
        state.push_back(event.clone());
        Ok(event.seq)
    }

    /// Waits for the next event.
    ///
    /// Returns `Ok(None)` when the timeout expires. A closed queue still
    /// delivers events that were queued before it closed.
    pub fn next_event(&self, timeout: Duration) -> Result<Option<Event>, EventError> {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.events.is_empty() && state.closed {
            return Err(EventError::Closed);
        }
        if state.events.is_empty() && !timeout.is_zero() {
            let (mut state, _) = self
                .available
                .wait_timeout(state, timeout)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.events.is_empty() {
                if state.closed {
                    return Err(EventError::Closed);
                }
                return Ok(None);
            }
            return Ok(state.events.pop_front());
        }
        if let Some(event) = state.events.pop_front() {
            self.available.notify_all();
            return Ok(Some(event));
        }
        Ok(None)
    }

    /// Closes the queue and wakes all waiters.
    pub fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.closed = true;
        self.available.notify_all();
    }

    /// Number of queued events.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .events
            .len()
    }

    /// Whether the queue has been closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .closed
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

fn make_event(state: &mut QueueState, pending: PendingEvent) -> Result<Event, EventError> {
    let event = Event {
        seq: state.next_seq,
        timestamp_ms: now_ms(),
        event_type: pending.event_type,
        op_id: pending.op_id,
        download_id: pending.download_id,
        data: pending.data,
        class: pending.class,
    };
    let encoded = event.encode()?;
    if encoded.len() > MAX_EVENT_BYTES {
        return Err(EventError::TooLarge {
            actual: encoded.len(),
            max: MAX_EVENT_BYTES,
        });
    }
    state.next_seq = state.next_seq.saturating_add(1);
    Ok(event)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use nexus_crypto::Value;

    use super::{EVENT_QUEUE_CAPACITY, EventClass, EventError, EventQueue, PendingEvent};

    fn state_event(name: &str) -> PendingEvent {
        PendingEvent::durable(
            "core.state",
            Value::Map(vec![(Value::Text("state".to_owned()), Value::Text(name.to_owned()))]),
        )
    }

    #[test]
    fn assigns_monotonic_sequence_numbers() {
        let queue = EventQueue::new();
        assert_eq!(queue.push(state_event("starting")).expect("push"), 1);
        assert_eq!(queue.push(state_event("running")).expect("push"), 2);
        let first = queue
            .next_event(Duration::from_millis(1))
            .expect("next")
            .expect("event");
        let second = queue
            .next_event(Duration::from_millis(1))
            .expect("next")
            .expect("event");
        assert_eq!(first.seq, 1);
        assert_eq!(second.seq, 2);
        assert!(first.timestamp_ms > 0);
    }

    #[test]
    fn times_out_without_an_event() {
        let queue = EventQueue::new();
        assert!(queue.next_event(Duration::from_millis(5)).expect("timeout").is_none());
    }

    #[test]
    fn progress_is_coalesced_per_operation() {
        let queue = EventQueue::new();
        for step in 1..=5 {
            queue
                .push(
                    PendingEvent::progress(
                        "test.progress",
                        Value::Map(vec![(Value::Text("step".to_owned()), Value::Unsigned(step))]),
                    )
                    .with_op(7),
                )
                .expect("progress");
        }
        assert_eq!(queue.depth(), 1);
        let event = queue
            .next_event(Duration::from_millis(1))
            .expect("next")
            .expect("event");
        assert_eq!(event.seq, 5);
        assert_eq!(event.class, EventClass::Progress);
    }

    #[test]
    fn durable_events_evict_progress_instead_of_blocking() {
        let queue = EventQueue::new();
        for index in 0..EVENT_QUEUE_CAPACITY {
            let event = if index == 0 {
                PendingEvent::progress(
                    "test.progress",
                    Value::Map(vec![(Value::Text("step".to_owned()), Value::Unsigned(1))]),
                )
            } else {
                state_event("running")
            };
            queue.push(event).expect("push");
        }
        queue.push(state_event("stopping")).expect("durable push");
        assert_eq!(queue.depth(), EVENT_QUEUE_CAPACITY);
    }

    #[test]
    fn nonblocking_push_refuses_when_only_durable_events_fill_the_queue() {
        let queue = EventQueue::new();
        for _ in 0..EVENT_QUEUE_CAPACITY {
            queue.push(state_event("running")).expect("durable");
        }
        assert!(matches!(queue.try_push(state_event("stopping")), Err(EventError::Full)));
    }

    #[test]
    fn close_wakes_waiter_and_reports_closed() {
        let queue = EventQueue::new();
        queue.close();
        assert!(matches!(
            queue.next_event(Duration::from_millis(1)),
            Err(EventError::Closed)
        ));
        assert!(queue.is_closed());
    }
}
