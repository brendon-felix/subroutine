use simple_core::ChangeEvent;
use sqlx::PgPool;
use tokio::sync::broadcast;

/// Capacity of the change-event broadcast channel.
/// If a slow client hasn't consumed 64 events the oldest will be dropped,
/// which is fine — the client just needs to know "something changed".
const BROADCAST_CAPACITY: usize = 64;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    /// Send a `ChangeEvent` here after every successful mutation.
    /// Each SSE client holds its own `Receiver` cloned from this sender.
    pub changes: broadcast::Sender<ChangeEvent>,
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        let (changes, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self { pool, changes }
    }
}
