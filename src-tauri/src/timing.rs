//! Best-effort local diagnostics. Playback callers never perform log file I/O or wait
//! for the writer; a full queue drops events and reports the count on the next record.
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    time::Instant,
};

const MAX_BYTES: u64 = 5 * 1024 * 1024;
static SENDER: OnceLock<SyncSender<Event>> = OnceLock::new();
static SEQUENCE: AtomicU64 = AtomicU64::new(1);
static DROPPED: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize)]
struct Event {
    timestamp_ms: i64,
    process_id: u32,
    thread_id: String,
    version: &'static str,
    span_id: u64,
    operation: &'static str,
    context: String,
    stage: &'static str,
    event: &'static str,
    elapsed_ms: u64,
    stage_ms: u64,
    dropped_events: u64,
}

pub(crate) fn initialize(directory: &Path) {
    let path = directory.join("aurora-timing.jsonl");
    let (sender, receiver) = mpsc::sync_channel(2048);
    if SENDER.get().is_some() {
        return;
    }
    if std::thread::Builder::new()
        .name("aurora-timing-writer".into())
        .spawn(move || {
            for event in receiver {
                if write_event(&path, &event, MAX_BYTES).is_err() {
                    DROPPED.fetch_add(1 + event.dropped_events, Ordering::Relaxed);
                }
            }
        })
        .is_ok()
    {
        let _ = SENDER.set(sender);
        Span::new("diagnostics.startup", "").finish(true);
    }
}

fn previous_path(path: &Path) -> PathBuf {
    path.with_extension("previous.jsonl")
}

fn write_event(path: &Path, event: &Event, max_bytes: u64) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(event)?;
    bytes.push(b'\n');
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::metadata(path).is_ok_and(|metadata| metadata.len() + bytes.len() as u64 > max_bytes) {
        let previous = previous_path(path);
        match fs::remove_file(&previous) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
        fs::rename(path, previous)?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(&bytes)
}

fn enqueue(sender: &SyncSender<Event>, mut event: Event, dropped: &AtomicU64) {
    event.dropped_events = dropped.swap(0, Ordering::Relaxed);
    if let Err(error) = sender.try_send(event) {
        let (mpsc::TrySendError::Full(event) | mpsc::TrySendError::Disconnected(event)) = error;
        dropped.fetch_add(1 + event.dropped_events, Ordering::Relaxed);
    }
}

pub(crate) struct Span {
    sender: Option<SyncSender<Event>>,
    id: u64,
    operation: &'static str,
    context: String,
    start: Instant,
    stage_start: Instant,
    stage: &'static str,
    outcome: &'static str,
}

impl Span {
    pub(crate) fn new(operation: &'static str, context: &str) -> Self {
        Self::with_sender(operation, context, SENDER.get().cloned())
    }

    fn with_sender(
        operation: &'static str,
        context: &str,
        sender: Option<SyncSender<Event>>,
    ) -> Self {
        let now = Instant::now();
        let span = Self {
            sender,
            id: SEQUENCE.fetch_add(1, Ordering::Relaxed),
            operation,
            context: context.chars().take(512).collect(),
            start: now,
            stage_start: now,
            stage: "start",
            outcome: "returned_without_success",
        };
        span.emit("begin");
        span
    }

    pub(crate) fn stage(&mut self, stage: &'static str) {
        self.emit("stage_end");
        self.stage = stage;
        self.stage_start = Instant::now();
        self.emit("stage_begin");
    }

    pub(crate) fn finish(&mut self, success: bool) {
        self.outcome = if success { "success" } else { "error" };
    }

    fn emit(&self, event: &'static str) {
        if let Some(sender) = &self.sender {
            enqueue(
                sender,
                Event {
                    timestamp_ms: crate::state_sync::now_ms(),
                    process_id: std::process::id(),
                    thread_id: format!("{:?}", std::thread::current().id()),
                    version: env!("CARGO_PKG_VERSION"),
                    span_id: self.id,
                    operation: self.operation,
                    context: self.context.clone(),
                    stage: self.stage,
                    event,
                    elapsed_ms: self.start.elapsed().as_millis() as u64,
                    stage_ms: self.stage_start.elapsed().as_millis() as u64,
                    dropped_events: 0,
                },
                &DROPPED,
            );
        }
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        self.emit(if std::thread::panicking() {
            "panic"
        } else {
            self.outcome
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(context: &str) -> Event {
        Event {
            timestamp_ms: 1,
            process_id: 1,
            thread_id: "test".into(),
            version: "test",
            span_id: 1,
            operation: "test",
            context: context.into(),
            stage: "copy",
            event: "begin",
            elapsed_ms: 0,
            stage_ms: 0,
            dropped_events: 0,
        }
    }

    #[test]
    fn slow_stage_and_early_return_are_correlated_in_written_trace() {
        let (sender, receiver) = mpsc::sync_channel(16);
        let span_id;
        {
            let mut span = Span::with_sender("shortcut.playback", "next", Some(sender));
            span_id = span.id;
            span.stage("playback_lock_wait");
            // Simulate a long wait without slowing down the test suite.
            span.start -= std::time::Duration::from_secs(20);
            span.stage_start -= std::time::Duration::from_secs(20);
            span.stage("playback_lock_held");
            // An early ? return must leave the unfinished stage and outcome visible.
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("trace.jsonl");
        for event in receiver {
            write_event(&path, &event, MAX_BYTES).unwrap();
        }
        let records: Vec<serde_json::Value> = fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(records.iter().all(|record| record["span_id"] == span_id));
        let wait = records
            .iter()
            .find(|record| {
                record["stage"] == "playback_lock_wait" && record["event"] == "stage_end"
            })
            .unwrap();
        assert!(wait["stage_ms"].as_u64().unwrap() >= 20_000);
        let last = records.last().unwrap();
        assert_eq!(last["stage"], "playback_lock_held");
        assert_eq!(last["event"], "returned_without_success");
    }

    #[test]
    fn full_queue_drops_without_waiting_and_reports_loss() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let dropped = AtomicU64::new(0);
        enqueue(&sender, event("first"), &dropped);
        enqueue(&sender, event("second"), &dropped);
        enqueue(&sender, event("third"), &dropped);
        assert_eq!(receiver.try_recv().unwrap().dropped_events, 0);
        enqueue(&sender, event("fourth"), &dropped);
        assert_eq!(receiver.try_recv().unwrap().dropped_events, 2);
    }

    #[test]
    fn rotation_keeps_latest_two_files_and_json_escapes_context() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("timing.jsonl");
        for context in ["first", "second", "title\n\"quoted\""] {
            write_event(&path, &event(context), 1).unwrap();
        }
        let previous: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(previous_path(&path)).unwrap()).unwrap();
        assert_eq!(previous["context"], "second");
        let current = fs::read_to_string(path).unwrap();
        assert_eq!(current.lines().count(), 1);
        let value: serde_json::Value = serde_json::from_str(&current).unwrap();
        assert_eq!(value["context"], "title\n\"quoted\"");
    }
}
