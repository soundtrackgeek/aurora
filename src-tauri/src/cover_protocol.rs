//! Blocking artwork work stays off the WebView callback and has a fixed ceiling.
use crate::{
    artwork,
    cover_cache::{COVER_CACHE_BYTES, CoverCache},
};
use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender, TrySendError},
    },
    thread,
};
use tauri::{AppHandle, Manager, UriSchemeResponder, http};

const WORKERS: usize = 3;
const QUEUED_REQUESTS: usize = 256;

struct WorkerPool<T> {
    sender: SyncSender<T>,
}

impl<T: Send + 'static> WorkerPool<T> {
    fn new(
        count: usize,
        capacity: usize,
        process: impl Fn(T) + Send + Sync + 'static,
    ) -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let receiver = Arc::new(Mutex::new(receiver));
        let process = Arc::new(process);
        for number in 0..count {
            let receiver = Arc::clone(&receiver);
            let process = Arc::clone(&process);
            thread::Builder::new()
                .name(format!("aurora-cover-{number}"))
                .spawn(move || {
                    loop {
                        let job = {
                            receiver
                                .lock()
                                .unwrap_or_else(|error| error.into_inner())
                                .recv()
                        };
                        let Ok(job) = job else { break };
                        // Release the receiver before decoding so all workers can run.
                        if catch_unwind(AssertUnwindSafe(|| process(job))).is_err() {
                            eprintln!("Aurora cover worker recovered from a failed request");
                        }
                    }
                })?;
        }
        Ok(Self { sender })
    }

    fn submit(&self, job: T) -> Result<(), T> {
        self.sender.try_send(job).map_err(|error| match error {
            TrySendError::Full(job) | TrySendError::Disconnected(job) => job,
        })
    }
}

struct CoverRequest {
    app: AppHandle,
    request: http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
}

enum Job {
    Sweep,
    Request(Box<CoverRequest>),
}

pub(crate) struct CoverProtocol {
    workers: WorkerPool<Job>,
}

impl CoverProtocol {
    pub(crate) fn new(cache_root: std::path::PathBuf) -> io::Result<Self> {
        let cache = Arc::new(CoverCache::new(cache_root, COVER_CACHE_BYTES));
        let workers = WorkerPool::new(WORKERS, QUEUED_REQUESTS, move |job| match job {
            Job::Sweep => cache.sweep(),
            Job::Request(job) => {
                let response = catch_unwind(AssertUnwindSafe(|| {
                    artwork::handle_cover_request(&job.app, &job.request, &cache)
                }))
                .unwrap_or_else(|_| {
                    artwork::cover_error_response(http::StatusCode::INTERNAL_SERVER_ERROR)
                });
                job.responder.respond(response);
            }
        })?;
        // Enumerate/evict old thumbnails on a worker, never during UI setup.
        let _ = workers.submit(Job::Sweep);
        Ok(Self { workers })
    }
}

pub(crate) fn dispatch(
    app: &AppHandle,
    request: http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let job = Job::Request(Box::new(CoverRequest {
        app: app.clone(),
        request,
        responder,
    }));
    let result = if let Some(protocol) = app.try_state::<CoverProtocol>() {
        protocol.workers.submit(job)
    } else {
        Err(job)
    };
    if let Err(Job::Request(job)) = result {
        // Do not block the resource callback or spawn extra decode threads when
        // fast scrolling fills the queue. Failed images can be requested again.
        job.responder.respond(artwork::cover_error_response(
            http::StatusCode::SERVICE_UNAVAILABLE,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn workers_run_concurrently_and_reject_overflow_without_waiting() {
        let (started_tx, started_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let pool = WorkerPool::new(3, 2, move |(id, release): (usize, mpsc::Receiver<()>)| {
            started_tx.send(id).unwrap();
            release.recv().unwrap();
            finished_tx.send(id).unwrap();
        })
        .unwrap();
        let mut releases = Vec::new();
        for id in 0..3 {
            let (tx, rx) = mpsc::channel();
            releases.push(tx);
            assert!(pool.submit((id, rx)).is_ok());
            assert_eq!(started_rx.recv_timeout(Duration::from_secs(5)).unwrap(), id);
        }
        // All three workers are held by explicit gates; these two are queued.
        for id in 3..5 {
            let (tx, rx) = mpsc::channel();
            releases.push(tx);
            assert!(pool.submit((id, rx)).is_ok());
        }
        let (_tx, rx) = mpsc::channel();
        assert_eq!(pool.submit((5, rx)).unwrap_err().0, 5);
        for tx in releases {
            tx.send(()).unwrap();
        }
        let mut finished = (0..5)
            .map(|_| finished_rx.recv_timeout(Duration::from_secs(5)).unwrap())
            .collect::<Vec<_>>();
        finished.sort();
        assert_eq!(finished, vec![0, 1, 2, 3, 4]);
    }
}
