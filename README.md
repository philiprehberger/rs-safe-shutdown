# rs-safe-shutdown

[![CI](https://github.com/philiprehberger/rs-safe-shutdown/actions/workflows/ci.yml/badge.svg)](https://github.com/philiprehberger/rs-safe-shutdown/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/philiprehberger-safe-shutdown.svg)](https://crates.io/crates/philiprehberger-safe-shutdown)
[![GitHub release](https://img.shields.io/github/v/release/philiprehberger/rs-safe-shutdown)](https://github.com/philiprehberger/rs-safe-shutdown/releases)
[![Last updated](https://img.shields.io/github/last-commit/philiprehberger/rs-safe-shutdown)](https://github.com/philiprehberger/rs-safe-shutdown/commits/main)
[![License](https://img.shields.io/github/license/philiprehberger/rs-safe-shutdown)](LICENSE)
[![Bug Reports](https://img.shields.io/github/issues/philiprehberger/rs-safe-shutdown/bug)](https://github.com/philiprehberger/rs-safe-shutdown/issues?q=is%3Aissue+is%3Aopen+label%3Abug)
[![Feature Requests](https://img.shields.io/github/issues/philiprehberger/rs-safe-shutdown/enhancement)](https://github.com/philiprehberger/rs-safe-shutdown/issues?q=is%3Aissue+is%3Aopen+label%3Aenhancement)
[![Sponsor](https://img.shields.io/badge/sponsor-GitHub%20Sponsors-ec6cb9)](https://github.com/sponsors/philiprehberger)

Graceful shutdown coordination with timeout support for Rust

## Installation

```toml
[dependencies]
philiprehberger-safe-shutdown = "0.1.8"
```

## Usage

```rust
use philiprehberger_safe_shutdown::{ShutdownSignal, ShutdownCoordinator, ShutdownResult};
use std::thread;
use std::time::Duration;

// Create a signal and coordinator
let signal = ShutdownSignal::new();
let coordinator = ShutdownCoordinator::new(signal.clone());

// Register a task — returns an RAII guard
let guard = coordinator.register("worker-1");

// Spawn work that listens for the signal
let sig = signal.clone();
let handle = thread::spawn(move || {
    // Simulate work, checking for shutdown
    while !sig.is_triggered() {
        thread::sleep(Duration::from_millis(10));
    }
    // Guard is dropped when the task finishes
    drop(guard);
});

// Initiate graceful shutdown with a timeout
let result = coordinator.shutdown(Duration::from_secs(5));
handle.join().unwrap();

match result {
    ShutdownResult::Completed => println!("All tasks finished cleanly"),
    ShutdownResult::TimedOut { pending } => {
        println!("Timed out waiting for: {:?}", pending);
    }
}
```

## API

| Type | Description |
|---|---|
| `ShutdownSignal` | Thread-safe trigger/wait signal. Clone to share across threads. |
| `ShutdownCoordinator` | Tracks registered tasks and orchestrates graceful shutdown with a timeout. |
| `ShutdownGuard` | RAII guard returned by `register()`. Automatically marks a task as complete on drop. |
| `ShutdownResult` | Enum: `Completed` (all tasks finished) or `TimedOut { pending: Vec<String> }`. |


## Development

```bash
cargo test
cargo clippy -- -D warnings
```

## Support

If you find this package useful, consider giving it a star on GitHub — it helps motivate continued maintenance and development.

[![LinkedIn](https://img.shields.io/badge/Philip%20Rehberger-LinkedIn-0A66C2?logo=linkedin)](https://www.linkedin.com/in/philiprehberger)
[![More packages](https://img.shields.io/badge/more-open%20source%20packages-blue)](https://philiprehberger.com/open-source-packages)

## License

[MIT](LICENSE)
