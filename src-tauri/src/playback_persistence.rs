//! Ordered, retryable playback writes. Queue locks protect memory only; the worker
//! never holds them during SQLite, filesystem, or history-mirror work.
use crate::{
    history::{HistoryCheckpoint, HistoryStore},
    state_store::{StateStore, StoredPlaybackState},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(crate) enum PlaybackWrite {
    History(Box<HistoryCheckpoint>),
    State(StoredPlaybackState),
}

impl PlaybackWrite {
    fn supersedes(&self, previous: &Self) -> bool {
        match (self, previous) {
            (Self::State(_), Self::State(_)) => true,
            (Self::History(next), Self::History(previous)) => {
                next.session_id() == previous.session_id()
            }
            _ => false,
        }
    }
}

struct PendingWrite {
    sequence: u64,
    write: PlaybackWrite,
    attempted: bool,
}

#[derive(Default)]
struct Queue {
    pending: VecDeque<PendingWrite>,
    accepted: u64,
    completed: u64,
    error: Option<String>,
    closed: bool,
}

#[derive(Default)]
struct Shared {
    queue: Mutex<Queue>,
    changed: Condvar,
}

struct Handle {
    shared: Arc<Shared>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        let mut queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.closed = true;
        self.shared.changed.notify_all();
    }
}

#[derive(Clone)]
pub(crate) struct PlaybackPersistence {
    handle: Arc<Handle>,
}

impl PlaybackPersistence {
    pub(crate) fn new(history: HistoryStore, state: StateStore) -> Result<Self, String> {
        Self::start(move |write| match write {
            PlaybackWrite::History(checkpoint) => history.persist_checkpoint(checkpoint),
            PlaybackWrite::State(snapshot) => state.save(snapshot),
        })
    }

    pub(crate) fn start(
        mut persist: impl FnMut(&PlaybackWrite) -> Result<(), String> + Send + 'static,
    ) -> Result<Self, String> {
        let shared = Arc::new(Shared::default());
        let worker = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("aurora-playback-persistence".to_owned())
            .spawn(move || {
                let mut retry_delay = Duration::from_millis(100);
                loop {
                    let mut pending = {
                        let mut queue = worker.queue.lock().unwrap_or_else(|e| e.into_inner());
                        while queue.pending.is_empty() && !queue.closed {
                            queue = worker
                                .changed
                                .wait(queue)
                                .unwrap_or_else(|e| e.into_inner());
                        }
                        let Some(pending) = queue.pending.pop_front() else {
                            return;
                        };
                        pending
                    };
                    let mut timing = crate::timing::Span::new("playback.persistence", "");
                    timing.stage(match &pending.write {
                        PlaybackWrite::History(_) => "history_write",
                        PlaybackWrite::State(_) => "state_write",
                    });
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        persist(&pending.write)
                    })).unwrap_or_else(|_| Err("Aurora's playback writer panicked; its pending write was retained for retry.".to_owned()));
                    timing.finish(result.is_ok());
                    let mut queue = worker.queue.lock().unwrap_or_else(|e| e.into_inner());
                    match result {
                        Ok(()) => {
                            queue.completed = pending.sequence;
                            queue.error = None;
                            retry_delay = Duration::from_millis(100);
                            worker.changed.notify_all();
                        }
                        Err(error) => {
                            // The failed event remains first. New sessions and state
                            // snapshots cannot overtake it or silently consume a retry.
                            queue.error = Some(error);
                            pending.attempted = true;
                            queue.pending.push_front(pending);
                            worker.changed.notify_all();
                            if queue.closed {
                                return;
                            }
                            drop(queue);
                            std::thread::sleep(retry_delay);
                            retry_delay = (retry_delay * 2).min(Duration::from_secs(5));
                        }
                    }
                }
            })
            .map_err(|error| {
                format!("Could not start Aurora's playback persistence worker: {error}")
            })?;
        Ok(Self {
            handle: Arc::new(Handle { shared }),
        })
    }

    pub(crate) fn enqueue(&self, write: PlaybackWrite) -> Result<(), String> {
        let shared = &self.handle.shared;
        let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        if queue.closed {
            return Err("Aurora's playback persistence queue is closed.".to_owned());
        }
        queue.accepted += 1;
        let pending = PendingWrite {
            sequence: queue.accepted,
            write,
            attempted: false,
        };
        // Only replace the unsent tail. An in-flight write is never mutated, and
        // snapshots from different sessions remain in their original order.
        if queue
            .pending
            .back()
            .is_some_and(|tail| !tail.attempted && pending.write.supersedes(&tail.write))
        {
            *queue.pending.back_mut().expect("tail exists") = pending;
        } else {
            queue.pending.push_back(pending);
        }
        shared.changed.notify_all();
        Ok(())
    }

    pub(crate) fn error(&self) -> Option<String> {
        self.handle
            .shared
            .queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .error
            .clone()
    }

    /// A barrier for every write accepted before this call, including coalesced
    /// checkpoints. Used outside the playback lock when closing/updating Aurora.
    pub(crate) fn flush(&self, timeout: Duration) -> Result<(), String> {
        let shared = &self.handle.shared;
        let deadline = Instant::now() + timeout;
        let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        let target = queue.accepted;
        while queue.completed < target {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!(
                    "Aurora still has playback/history updates waiting to be saved. {}",
                    queue
                        .error
                        .as_deref()
                        .unwrap_or("The storage operation has not finished yet.")
                ));
            }
            queue = shared
                .changed
                .wait_timeout(queue, remaining)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn state(position: f64) -> PlaybackWrite {
        PlaybackWrite::State(StoredPlaybackState {
            position_seconds: position,
            ..Default::default()
        })
    }

    #[test]
    fn blocked_writer_accepts_work_coalesces_unsent_state_and_retains_it_after_flush_timeout() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&seen);
        let mut first = true;
        let writer = PlaybackPersistence::start(move |write| {
            if first {
                first = false;
                started_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            if let PlaybackWrite::State(state) = write {
                recorded.lock().unwrap().push(state.position_seconds);
            }
            Ok(())
        })
        .unwrap();
        writer.enqueue(state(1.0)).unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        writer.enqueue(state(2.0)).unwrap();
        writer.enqueue(state(3.0)).unwrap();
        assert!(writer.flush(Duration::ZERO).is_err());
        assert!(seen.lock().unwrap().is_empty());
        release_tx.send(()).unwrap();
        writer.flush(Duration::from_secs(5)).unwrap();
        assert_eq!(*seen.lock().unwrap(), vec![1.0, 3.0]);
    }

    #[test]
    fn failed_write_is_retried_before_newer_state_and_recovery_clears_error() {
        let (failed_tx, failed_rx) = mpsc::channel();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&seen);
        let mut first = true;
        let writer = PlaybackPersistence::start(move |write| {
            if let PlaybackWrite::State(state) = write {
                recorded.lock().unwrap().push(state.position_seconds);
            }
            if first {
                first = false;
                failed_tx.send(()).unwrap();
                return Err("storage temporarily busy".to_owned());
            }
            Ok(())
        })
        .unwrap();
        writer.enqueue(state(1.0)).unwrap();
        failed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        writer.enqueue(state(2.0)).unwrap();
        writer.flush(Duration::from_secs(5)).unwrap();
        assert_eq!(*seen.lock().unwrap(), vec![1.0, 1.0, 2.0]);
        assert!(writer.error().is_none());
    }
}
