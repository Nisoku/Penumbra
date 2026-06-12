use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub mod thread {
    #[cfg(not(target_arch = "wasm32"))]
    mod imp {
        pub use std::thread::*;
    }
    #[cfg(target_arch = "wasm32")]
    mod imp {
        pub use wasm_thread::*;
    }
    pub use imp::*;
}

pub use crossbeam_channel as channel;

/// A managed worker handle with a stop signal.
pub struct Worker {
    cancel: Arc<AtomicBool>,
}

impl Worker {
    /// Returns true if a stop has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Request the worker to stop.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Spawn a named background thread that can be cancelled.
///
/// Returns a [`Worker`] whose `cancel()` sets the flag the closure checks via
/// `is_cancelled()`. The thread's [`JoinHandle`](std::thread::JoinHandle) is
/// detached and the worker runs until `f` returns or panics.
pub fn spawn_worker<F>(name: &str, f: F) -> Worker
where
    F: FnOnce(Worker) + Send + 'static,
{
    let cancel = Arc::new(AtomicBool::new(false));
    let worker = Worker {
        cancel: Arc::clone(&cancel),
    };

    thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let w = Worker { cancel };
            f(w);
        })
        .expect("failed to spawn worker thread");

    worker
}

/// Spawn a named thread and detach it immediately.
pub fn spawn_detached<F>(name: &str, f: F)
where
    F: FnOnce() + Send + 'static,
{
    thread::Builder::new()
        .name(name.into())
        .spawn(f)
        .expect("failed to spawn detached thread");
}
