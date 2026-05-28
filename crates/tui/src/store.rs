//! HTTP-backed store for the TUI.
//!
//! Mimics the API the TUI expects from the old `simple_core::DatabaseStore`:
//! `new(url, on_change)`, `status()`, `all_actions()`, `all_events()`, and
//! the mutating fire-and-forget methods.

use std::sync::{Arc, Mutex};

use serde::Deserialize;
use simple_core::{Action, Event};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum StoreStatus {
    Loading,
    Ready,
    Error(String),
}

#[derive(Deserialize)]
struct AllData {
    actions: Vec<Action>,
    events: Vec<Event>,
}

struct StoreInner {
    status: StoreStatus,
    actions: Vec<Action>,
    events: Vec<Event>,
}

#[derive(Clone)]
pub struct DatabaseStore {
    inner: Arc<Mutex<StoreInner>>,
    client: Arc<reqwest::Client>,
    base_url: Arc<String>,
}

impl DatabaseStore {
    /// Create the store and immediately spawn a background task to load all
    /// data.  `on_change` is called once the initial fetch completes (or fails).
    pub fn new(base_url: String, on_change: impl Fn() + Send + 'static) -> Self {
        let inner = Arc::new(Mutex::new(StoreInner {
            status: StoreStatus::Loading,
            actions: Vec::new(),
            events: Vec::new(),
        }));
        let client = Arc::new(reqwest::Client::new());
        let base_url = Arc::new(base_url);

        let inner_clone = Arc::clone(&inner);
        let client_clone = Arc::clone(&client);
        let base_clone = Arc::clone(&base_url);

        tokio::spawn(async move {
            match client_clone
                .get(format!("{}/api/data", base_clone))
                .send()
                .await
                .and_then(|r| r.error_for_status())
            {
                Ok(resp) => match resp.json::<AllData>().await {
                    Ok(data) => {
                        let mut lock = inner_clone.lock().unwrap();
                        lock.actions = data.actions;
                        lock.events = data.events;
                        lock.status = StoreStatus::Ready;
                    }
                    Err(e) => {
                        inner_clone.lock().unwrap().status = StoreStatus::Error(e.to_string());
                    }
                },
                Err(e) => {
                    inner_clone.lock().unwrap().status = StoreStatus::Error(e.to_string());
                }
            }
            on_change();
        });

        Self {
            inner,
            client,
            base_url,
        }
    }

    pub fn status(&self) -> StoreStatus {
        self.inner.lock().unwrap().status.clone()
    }

    pub fn all_actions(&self) -> Vec<Action> {
        self.inner.lock().unwrap().actions.clone()
    }

    pub fn all_events(&self) -> Vec<Event> {
        self.inner.lock().unwrap().events.clone()
    }

    // ── Write API (optimistic local update + fire-and-forget HTTP) ────────────

    pub fn upsert_action(&self, action: Action) {
        {
            let mut lock = self.inner.lock().unwrap();
            if let Some(pos) = lock.actions.iter().position(|a| a.id == action.id) {
                lock.actions[pos] = action.clone();
            } else {
                lock.actions.push(action.clone());
            }
        }
        let client = Arc::clone(&self.client);
        let base = Arc::clone(&self.base_url);
        tokio::spawn(async move {
            let _ = client
                .put(format!("{}/api/actions/{}", base, action.id))
                .json(&action)
                .send()
                .await;
        });
    }

    pub fn trash_action(&self, id: Uuid) {
        self.inner.lock().unwrap().actions.retain(|a| a.id != id);
        let client = Arc::clone(&self.client);
        let base = Arc::clone(&self.base_url);
        tokio::spawn(async move {
            let _ = client
                .delete(format!("{}/api/actions/{id}", base))
                .send()
                .await;
        });
    }

    pub fn complete_action(&self, id: Uuid) {
        // Optimistic: mark completed locally with current timestamp.
        {
            let mut lock = self.inner.lock().unwrap();
            if let Some(action) = lock.actions.iter_mut().find(|a| a.id == id) {
                action.complete(chrono::Utc::now());
            }
        }
        let client = Arc::clone(&self.client);
        let base = Arc::clone(&self.base_url);
        tokio::spawn(async move {
            let _ = client
                .post(format!("{}/api/actions/{id}/complete", base))
                .send()
                .await;
        });
    }

    pub fn upsert_event(&self, event: Event) {
        {
            let mut lock = self.inner.lock().unwrap();
            if let Some(pos) = lock.events.iter().position(|e| e.id == event.id) {
                lock.events[pos] = event.clone();
            } else {
                lock.events.push(event.clone());
            }
        }
        let client = Arc::clone(&self.client);
        let base = Arc::clone(&self.base_url);
        tokio::spawn(async move {
            let _ = client
                .put(format!("{}/api/events/{}", base, event.id))
                .json(&event)
                .send()
                .await;
        });
    }

    pub fn trash_event(&self, id: Uuid) {
        self.inner.lock().unwrap().events.retain(|e| e.id != id);
        let client = Arc::clone(&self.client);
        let base = Arc::clone(&self.base_url);
        tokio::spawn(async move {
            let _ = client
                .delete(format!("{}/api/events/{id}", base))
                .send()
                .await;
        });
    }
}
