//! Graceful shutdown coordination with timeout support.
//!
//! # Example
//!
//! ```rust
//! use philiprehberger_safe_shutdown::{ShutdownSignal, ShutdownCoordinator};
//!
//! let signal = ShutdownSignal::new();
//! let coordinator = ShutdownCoordinator::new(signal.clone());
//! let _guard = coordinator.register("http");
//! ```

use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// A thread-safe shutdown signal that supports trigger/wait semantics.
///
/// Clone this to share the same signal across multiple threads.
pub struct ShutdownSignal {
    inner: Arc<SignalInner>,
}

struct SignalInner {
    triggered: Mutex<bool>,
    condvar: Condvar,
}

impl ShutdownSignal {
    /// Creates a new signal in the non-triggered state.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SignalInner {
                triggered: Mutex::new(false),
                condvar: Condvar::new(),
            }),
        }
    }

    /// Triggers the shutdown signal, waking all waiters.
    pub fn trigger(&self) {
        let mut triggered = self.inner.triggered.lock().unwrap();
        *triggered = true;
        self.inner.condvar.notify_all();
    }

    /// Returns `true` if the signal has been triggered.
    pub fn is_triggered(&self) -> bool {
        *self.inner.triggered.lock().unwrap()
    }

    /// Blocks the current thread until the signal is triggered.
    pub fn wait(&self) {
        let triggered = self.inner.triggered.lock().unwrap();
        let _guard = self
            .inner
            .condvar
            .wait_while(triggered, |t| !*t)
            .unwrap();
    }
}

impl Clone for ShutdownSignal {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Default for ShutdownSignal {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ShutdownSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShutdownSignal")
            .field("triggered", &self.is_triggered())
            .finish()
    }
}

/// The result of a shutdown operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownResult {
    /// All registered tasks completed before the timeout.
    Completed,
    /// Some tasks did not complete before the timeout.
    TimedOut {
        /// Names of tasks that were still pending when the timeout expired.
        pending: Vec<String>,
    },
}

/// Coordinates graceful shutdown by tracking registered tasks via RAII guards.
pub struct ShutdownCoordinator {
    signal: ShutdownSignal,
    tasks: Arc<Mutex<HashSet<String>>>,
}

impl ShutdownCoordinator {
    /// Creates a new coordinator linked to the given signal.
    pub fn new(signal: ShutdownSignal) -> Self {
        Self {
            signal,
            tasks: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Registers a named task and returns an RAII guard. When the guard is
    /// dropped, the task is automatically marked as complete.
    pub fn register(&self, name: impl Into<String>) -> ShutdownGuard {
        let name = name.into();
        self.tasks.lock().unwrap().insert(name.clone());
        ShutdownGuard {
            name,
            tasks: Arc::clone(&self.tasks),
        }
    }

    /// Triggers the shutdown signal and waits up to `timeout` for all
    /// registered tasks to complete. Returns whether all tasks finished
    /// or which ones were still pending.
    pub fn shutdown(&self, timeout: Duration) -> ShutdownResult {
        self.signal.trigger();

        let start = Instant::now();
        loop {
            {
                let tasks = self.tasks.lock().unwrap();
                if tasks.is_empty() {
                    return ShutdownResult::Completed;
                }
            }
            if start.elapsed() >= timeout {
                let tasks = self.tasks.lock().unwrap();
                let mut pending: Vec<String> = tasks.iter().cloned().collect();
                pending.sort();
                return ShutdownResult::TimedOut { pending };
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Returns a sorted list of currently pending task names.
    pub fn pending_tasks(&self) -> Vec<String> {
        let tasks = self.tasks.lock().unwrap();
        let mut names: Vec<String> = tasks.iter().cloned().collect();
        names.sort();
        names
    }
}

impl fmt::Debug for ShutdownCoordinator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShutdownCoordinator")
            .field("signal", &self.signal)
            .field("pending_tasks", &self.pending_tasks())
            .finish()
    }
}

/// An RAII guard that automatically deregisters a task when dropped.
///
/// Each guard is unique and cannot be cloned.
pub struct ShutdownGuard {
    name: String,
    tasks: Arc<Mutex<HashSet<String>>>,
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        self.tasks.lock().unwrap().remove(&self.name);
    }
}

impl fmt::Debug for ShutdownGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShutdownGuard")
            .field("name", &self.name)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn signal_trigger() {
        let signal = ShutdownSignal::new();
        assert!(!signal.is_triggered());
        signal.trigger();
        assert!(signal.is_triggered());
    }

    #[test]
    fn signal_wait() {
        let signal = ShutdownSignal::new();
        let sig = signal.clone();
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            sig.trigger();
        });
        signal.wait();
        assert!(signal.is_triggered());
        handle.join().unwrap();
    }

    #[test]
    fn signal_clone() {
        let signal = ShutdownSignal::new();
        let cloned = signal.clone();
        signal.trigger();
        assert!(cloned.is_triggered());
    }

    #[test]
    fn coordinator_completed() {
        let signal = ShutdownSignal::new();
        let coordinator = ShutdownCoordinator::new(signal);
        let guard = coordinator.register("task-1");
        drop(guard);
        let result = coordinator.shutdown(Duration::from_millis(100));
        assert_eq!(result, ShutdownResult::Completed);
    }

    #[test]
    fn coordinator_timeout() {
        let signal = ShutdownSignal::new();
        let coordinator = ShutdownCoordinator::new(signal);
        let _guard = coordinator.register("task-1");
        let result = coordinator.shutdown(Duration::from_millis(50));
        assert_eq!(
            result,
            ShutdownResult::TimedOut {
                pending: vec!["task-1".to_string()]
            }
        );
    }

    #[test]
    fn pending_tasks() {
        let signal = ShutdownSignal::new();
        let coordinator = ShutdownCoordinator::new(signal);
        let _g1 = coordinator.register("beta");
        let _g2 = coordinator.register("alpha");
        let pending = coordinator.pending_tasks();
        assert_eq!(pending, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn guard_drop() {
        let signal = ShutdownSignal::new();
        let coordinator = ShutdownCoordinator::new(signal);
        let guard = coordinator.register("task-1");
        assert_eq!(coordinator.pending_tasks().len(), 1);
        drop(guard);
        assert!(coordinator.pending_tasks().is_empty());
    }

    #[test]
    fn multiple_guards() {
        let signal = ShutdownSignal::new();
        let coordinator = ShutdownCoordinator::new(signal);

        let guards: Vec<_> = (0..5)
            .map(|i| coordinator.register(format!("task-{i}")))
            .collect();

        assert_eq!(coordinator.pending_tasks().len(), 5);
        drop(guards);

        let result = coordinator.shutdown(Duration::from_millis(100));
        assert_eq!(result, ShutdownResult::Completed);
    }

    #[test]
    fn coordinator_triggers_signal() {
        let signal = ShutdownSignal::new();
        let sig = signal.clone();
        let coordinator = ShutdownCoordinator::new(signal);
        assert!(!sig.is_triggered());
        let result = coordinator.shutdown(Duration::from_millis(50));
        assert_eq!(result, ShutdownResult::Completed);
        assert!(sig.is_triggered());
    }
}
