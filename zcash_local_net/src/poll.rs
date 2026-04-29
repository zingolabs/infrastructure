//! Async polling primitive shared by lifecycle waiters.
//!
//! Replaces the `std::thread::sleep` loops that previously parked the
//! tokio worker thread inside `async fn`s. See
//! zingolabs/infrastructure#251.

use std::time::Duration;

/// Polls `predicate` every `interval` until it returns `true`, or
/// returns `Err(Elapsed)` once `timeout` elapses. Async-native: built
/// on `tokio::time::timeout` and `tokio::time::sleep`, never
/// `std::thread::sleep`.
pub(crate) async fn poll_until<F, Fut>(
    interval: Duration,
    timeout: Duration,
    mut predicate: F,
) -> Result<(), tokio::time::error::Elapsed>
where
    F: FnMut() -> Fut + Send,
    Fut: std::future::Future<Output = bool> + Send,
{
    tokio::time::timeout(timeout, async move {
        loop {
            if predicate().await {
                return;
            }
            tokio::time::sleep(interval).await;
        }
    })
    .await
}
