use std::thread;

use chrono::{DateTime, Utc};
use flume;
use gpui::{App, AppContext, Context, Entity, EventEmitter, Global};
use simple_core::{
    Action, ActionState, AllData, AnyItem, CompleteResult, Event, Routine, next_queue_slot,
};
use uuid::Uuid;

// ── Status / events ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum StoreStatus {
    Loading,
    Ready,
    Error(String),
}

pub struct DatabaseError {
    pub _message: String,
}

pub struct DataChanged;
pub struct ActionDataChanged;
pub struct EventDataChanged;
pub struct RoutineDataChanged;

// ── Internal channel payload types ───────────────────────────────────────────

type Reply<T> = flume::Sender<Result<T, String>>;

enum Cmd {
    FetchAll(Reply<AllData>),
    // Simple CRUD
    UpsertAction(Action, Reply<()>),
    DeleteAction(Uuid, Reply<()>),
    UpsertEvent(Event, Reply<()>),
    DeleteEvent(Uuid, Reply<()>),
    UpsertRoutine(Routine, Reply<()>),
    DeleteRoutine(Uuid, Reply<()>),
    // Combined create + follow-up, sequenced atomically in the worker
    UpsertAndQueueAction(Action, Reply<Vec<Action>>),
    // UpsertAndScheduleEvent(Event, Reply<Event>),
    UpsertAndInstantiateRoutine(Routine, Option<DateTime<Utc>>, Reply<Vec<Action>>),
    // State transitions
    CompleteAction(Uuid, Reply<CompleteResult>),
    QueueAction(Uuid, Reply<Vec<Action>>),
    ClearActionDuration(Uuid, Reply<Action>),
    BacklogAction(Uuid, Reply<Action>),
    InstantiateRoutine(Uuid, Option<DateTime<Utc>>, Reply<Vec<Action>>),
    RefreshPipeline(Reply<Vec<Action>>),
    ExpeditePipeline(Reply<Vec<Action>>),
}

// ── Store ─────────────────────────────────────────────────────────────────────

pub struct AppDatabaseStore {
    cmd_tx: flume::Sender<Cmd>,
    _worker: thread::JoinHandle<()>,
    status: StoreStatus,
    actions: Vec<Action>,
    events: Vec<Event>,
    routines: Vec<Routine>,
}

impl AppDatabaseStore {
    fn new(server_url: String, cx: &mut Context<Self>) -> Self {
        tracing::info!(server = %server_url, "connecting to server");

        let (cmd_tx, cmd_rx) = flume::unbounded::<Cmd>();

        // Kick off the initial fetch before the worker even starts looping.
        let (init_tx, init_rx) = flume::bounded(1);
        let _ = cmd_tx.send(Cmd::FetchAll(init_tx));

        // One-shot channel: the SSE loop sends () whenever the server emits a
        // change event. The GPUI side uses this to trigger a re-fetch.
        let (sse_tx, sse_rx) = flume::unbounded::<()>();

        let base = server_url.clone();
        let worker = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(async move {
                let client = reqwest::Client::new();
                // Run the SSE listener and the cmd loop concurrently on the
                // same single-thread runtime. Neither blocks the other.
                let sse_client = client.clone();
                let sse_base = base.clone();
                tokio::join!(sse_loop(&sse_client, &sse_base, sse_tx), async {
                    while let Ok(cmd) = cmd_rx.recv_async().await {
                        run(&client, &base, cmd).await;
                    }
                });
            });
        });

        cx.spawn(async move |this, cx| {
            let Ok(result) = init_rx.recv_async().await else {
                return;
            };
            let _ = this.update(cx, |store, cx| match result {
                Ok(data) => {
                    tracing::info!(
                        actions = data.actions.len(),
                        events = data.events.len(),
                        routines = data.routines.len(),
                        "store ready"
                    );
                    store.actions = data.actions;
                    store.events = data.events;
                    store.routines = data.routines;
                    store.status = StoreStatus::Ready;
                    cx.emit(DataChanged);
                    cx.emit(ActionDataChanged);
                    cx.emit(EventDataChanged);
                    cx.emit(RoutineDataChanged);
                    cx.notify();
                }
                Err(e) => {
                    tracing::error!(error = %e, "initial fetch failed");
                    store.status = StoreStatus::Error(e.clone());
                    cx.emit(DatabaseError { _message: e });
                }
            });
        })
        .detach();

        // Persistent task: re-fetch all data whenever the SSE loop signals a
        // server-side change. Runs for the lifetime of the store entity.
        let cmd_tx_sse = cmd_tx.clone();
        cx.spawn(async move |this, cx| {
            loop {
                // Block until the SSE loop tells us something changed.
                if sse_rx.recv_async().await.is_err() {
                    break; // worker dropped — app is shutting down
                }
                tracing::debug!("SSE change event received, re-fetching");
                let (tx, rx) = flume::bounded(1);
                let _ = cmd_tx_sse.send(Cmd::FetchAll(tx));
                if let Ok(Ok(data)) = rx.recv_async().await {
                    let _ = this.update(cx, |store, cx| {
                        store.actions = data.actions;
                        store.events = data.events;
                        store.routines = data.routines;
                        cx.emit(DataChanged);
                        cx.emit(ActionDataChanged);
                        cx.emit(EventDataChanged);
                        cx.notify();
                    });
                }
            }
        })
        .detach();

        Self {
            cmd_tx,
            _worker: worker,
            status: StoreStatus::Loading,
            actions: vec![],
            events: vec![],
            routines: vec![],
        }
    }

    pub fn initialize_global(server_url: String, cx: &mut App) -> Entity<Self> {
        if cx.has_global::<GlobalStore>() {
            return cx.global::<GlobalStore>().0.clone();
        }
        let store = cx.new(|cx| Self::new(server_url, cx));
        cx.set_global(GlobalStore(store.clone()));
        store
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalStore>().0.clone()
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn upsert_action_local(&mut self, action: Action) {
        if let Some(pos) = self.actions.iter().position(|a| a.id == action.id) {
            self.actions[pos] = action;
        } else {
            self.actions.push(action);
        }
    }

    fn upsert_event_local(&mut self, event: Event) {
        if let Some(pos) = self.events.iter().position(|e| e.id == event.id) {
            self.events[pos] = event;
        } else {
            self.events.push(event);
        }
    }

    fn upsert_routine_local(&mut self, routine: Routine) {
        if let Some(pos) = self.routines.iter().position(|r| r.id == routine.id) {
            self.routines[pos] = routine;
        } else {
            self.routines.push(routine);
        }
    }

    // Sends a command and spawns a smol task that awaits the reply, then calls
    // `apply` on the entity.  Used for operations where the server response
    // carries data we need to merge (complete, queue, pipeline, etc.).
    fn dispatch<T: 'static>(
        &self,
        cmd: Cmd,
        rx: flume::Receiver<Result<T, String>>,
        cx: &mut Context<Self>,
        apply: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) {
        let _ = self.cmd_tx.send(cmd);
        cx.spawn(async move |this, cx| {
            match rx.recv_async().await {
                Ok(Ok(data)) => {
                    let _ = this.update(cx, |store, cx| apply(store, data, cx));
                }
                Ok(Err(e)) => tracing::error!("{e}"),
                Err(_) => {} // worker dropped
            }
        })
        .detach();
    }

    // ── Read API ──────────────────────────────────────────────────────────────

    // pub fn is_ready(&self) -> bool {
    //     self.status == StoreStatus::Ready
    // }

    pub fn actions(&self) -> Vec<Action> {
        self.actions.clone()
    }

    pub fn get_action(&self, id: Uuid) -> Option<Action> {
        self.actions.iter().find(|a| a.id == id).cloned()
    }

    // pub fn active_action(&self) -> Option<Action> {
    //     let now = chrono::Utc::now();
    //     self.actions
    //         .iter()
    //         .find(|a| matches!(a.state, ActionState::Queued(t) if t.time <= now))
    //         .cloned()
    // }

    pub fn events(&self) -> Vec<Event> {
        self.events.clone()
    }

    pub fn get_event(&self, id: Uuid) -> Option<Event> {
        self.events.iter().find(|e| e.id == id).cloned()
    }

    pub fn sorted_timeline(&self) -> Vec<AnyItem> {
        let mut items: Vec<AnyItem> = self
            .actions
            .iter()
            .filter(|a| a.is_queued() || a.is_completed())
            .map(|a| AnyItem::Action(a.clone()))
            .chain(self.events.iter().map(|e| AnyItem::Event(e.clone())))
            .collect();
        items.sort_by_key(|item| {
            item.time()
                .unwrap_or(chrono::DateTime::<chrono::Utc>::MAX_UTC)
        });
        items
    }

    pub fn sorted_queue(&self) -> Vec<AnyItem> {
        let now = chrono::Utc::now();
        let mut items: Vec<AnyItem> = self
            .actions
            .iter()
            .filter(|a| a.is_queued())
            .map(|a| AnyItem::Action(a.clone()))
            .chain(
                self.events
                    .iter()
                    .filter(|e| e.end_time() > now)
                    .map(|e| AnyItem::Event(e.clone())),
            )
            .collect();
        items.sort_by_key(|item| {
            item.time()
                .unwrap_or(chrono::DateTime::<chrono::Utc>::MAX_UTC)
        });
        items
    }

    pub fn backlogged_actions(&self) -> Vec<Action> {
        self.actions
            .iter()
            .filter(|a| a.is_backlogged())
            .cloned()
            .collect()
    }

    pub fn routines(&self) -> Vec<Routine> {
        self.routines.clone()
    }

    pub fn get_routine(&self, id: Uuid) -> Option<Routine> {
        self.routines.iter().find(|r| r.id == id).cloned()
    }

    // ── Write API — simple CRUD (optimistic) ──────────────────────────────────
    // Update local state immediately so the UI is responsive, then fire-and-
    // forget to the server.  Errors are logged; no rollback for now.

    pub fn upsert_action(&mut self, action: Action, cx: &mut Context<Self>) {
        self.upsert_action_local(action.clone());
        cx.emit(ActionDataChanged);
        cx.emit(DataChanged);
        cx.notify();

        let (tx, rx) = flume::bounded(1);
        self.dispatch(
            Cmd::UpsertAction(action, tx),
            rx,
            cx,
            |_store, _result, _cx| {},
        );
    }

    pub fn delete_action(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.actions.retain(|a| a.id != id);
        cx.emit(ActionDataChanged);
        cx.emit(DataChanged);
        cx.notify();

        let (tx, rx) = flume::bounded(1);
        self.dispatch(Cmd::DeleteAction(id, tx), rx, cx, |_store, _, _cx| {});
    }

    pub fn upsert_event(&mut self, event: Event, cx: &mut Context<Self>) {
        self.upsert_event_local(event.clone());
        cx.emit(EventDataChanged);
        cx.emit(DataChanged);
        cx.notify();

        let (tx, rx) = flume::bounded(1);
        self.dispatch(Cmd::UpsertEvent(event, tx), rx, cx, |_store, _, _cx| {});
    }

    pub fn delete_event(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.events.retain(|e| e.id != id);
        cx.emit(EventDataChanged);
        cx.emit(DataChanged);
        cx.notify();

        let (tx, rx) = flume::bounded(1);
        self.dispatch(Cmd::DeleteEvent(id, tx), rx, cx, |_store, _, _cx| {});
    }

    pub fn upsert_routine(&mut self, routine: Routine, cx: &mut Context<Self>) {
        self.upsert_routine_local(routine.clone());
        cx.emit(RoutineDataChanged);
        cx.emit(DataChanged);
        cx.notify();

        let (tx, rx) = flume::bounded(1);
        self.dispatch(Cmd::UpsertRoutine(routine, tx), rx, cx, |_store, _, _cx| {});
    }

    pub fn delete_routine(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.routines.retain(|r| r.id != id);
        cx.emit(RoutineDataChanged);
        cx.emit(DataChanged);
        cx.notify();

        let (tx, rx) = flume::bounded(1);
        self.dispatch(Cmd::DeleteRoutine(id, tx), rx, cx, |_store, _, _cx| {});
    }

    pub fn upsert_and_queue_action(&mut self, action: Action, cx: &mut Context<Self>) {
        let optimistic = if action.is_queued() {
            action.clone()
        } else {
            let slot = next_queue_slot(&self.actions, &self.events, chrono::Utc::now());
            action.clone().with_state(ActionState::queued(slot, false))
        };
        self.upsert_action_local(optimistic);
        cx.emit(ActionDataChanged);
        cx.emit(DataChanged);
        cx.notify();

        let (tx, rx) = flume::bounded(1);
        self.dispatch(
            Cmd::UpsertAndQueueAction(action, tx),
            rx,
            cx,
            |store, changed: Vec<Action>, cx| {
                for a in changed {
                    store.upsert_action_local(a);
                }
                cx.emit(ActionDataChanged);
                cx.emit(DataChanged);
                cx.notify();
            },
        );
    }

    // pub fn upsert_and_schedule_event(&mut self, event: Event, cx: &mut Context<Self>) {
    //     self.upsert_event_local(event.clone());
    //     cx.emit(EventDataChanged);
    //     cx.emit(DataChanged);
    //     cx.notify();

    //     let (tx, rx) = flume::bounded(1);
    //     self.dispatch(
    //         Cmd::UpsertAndScheduleEvent(event, tx),
    //         rx,
    //         cx,
    //         |store, event: Event, cx| {
    //             store.upsert_event_local(event);
    //             cx.emit(EventDataChanged);
    //             cx.emit(DataChanged);
    //             cx.notify();
    //         },
    //     );
    // }

    pub fn upsert_and_instantiate_routine(
        &mut self,
        routine: Routine,
        start_time: Option<DateTime<Utc>>,
        cx: &mut Context<Self>,
    ) {
        self.upsert_routine_local(routine.clone());
        cx.emit(RoutineDataChanged);
        cx.emit(DataChanged);
        cx.notify();

        let (tx, rx) = flume::bounded(1);
        self.dispatch(
            Cmd::UpsertAndInstantiateRoutine(routine, start_time, tx),
            rx,
            cx,
            |store, actions: Vec<Action>, cx| {
                for a in actions {
                    store.upsert_action_local(a);
                }
                cx.emit(ActionDataChanged);
                cx.emit(DataChanged);
                cx.notify();
            },
        );
    }

    pub fn complete_action(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let (tx, rx) = flume::bounded(1);
        self.dispatch(
            Cmd::CompleteAction(id, tx),
            rx,
            cx,
            |store, result: CompleteResult, cx| {
                store.upsert_action_local(result.completed);
                if let Some(next) = result.next {
                    store.upsert_action_local(next);
                }
                cx.emit(ActionDataChanged);
                cx.emit(DataChanged);
                cx.notify();
            },
        );
    }

    pub fn auto_queue_action(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let (tx, rx) = flume::bounded(1);
        self.dispatch(
            Cmd::QueueAction(id, tx),
            rx,
            cx,
            |store, changed: Vec<Action>, cx| {
                for action in changed {
                    store.upsert_action_local(action);
                }
                cx.emit(ActionDataChanged);
                cx.emit(DataChanged);
                cx.notify();
            },
        );
    }

    pub fn clear_action_duration(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let (tx, rx) = flume::bounded(1);
        self.dispatch(
            Cmd::ClearActionDuration(id, tx),
            rx,
            cx,
            |store, action: Action, cx| {
                store.upsert_action_local(action);
                cx.emit(ActionDataChanged);
                cx.emit(DataChanged);
                cx.notify();
            },
        );
    }

    pub fn backlog_action(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let (tx, rx) = flume::bounded(1);
        self.dispatch(
            Cmd::BacklogAction(id, tx),
            rx,
            cx,
            |store, action: Action, cx| {
                store.upsert_action_local(action);
                cx.emit(ActionDataChanged);
                cx.emit(DataChanged);
                cx.notify();
            },
        );
    }

    pub fn refresh_pipeline(&mut self, cx: &mut Context<Self>) {
        let (tx, rx) = flume::bounded(1);
        self.dispatch(
            Cmd::RefreshPipeline(tx),
            rx,
            cx,
            |store, changed: Vec<Action>, cx| {
                for action in changed {
                    store.upsert_action_local(action);
                }
                cx.emit(ActionDataChanged);
                cx.emit(EventDataChanged);
                cx.emit(DataChanged);
                cx.notify();
            },
        );
    }

    pub fn expedite_actions(&mut self, cx: &mut Context<Self>) {
        let (tx, rx) = flume::bounded(1);
        self.dispatch(
            Cmd::ExpeditePipeline(tx),
            rx,
            cx,
            |store, changed: Vec<Action>, cx| {
                for action in changed {
                    store.upsert_action_local(action);
                }
                cx.emit(ActionDataChanged);
                cx.emit(DataChanged);
                cx.notify();
            },
        );
    }

    pub fn instantiate_routine(
        &mut self,
        id: Uuid,
        start_time: Option<DateTime<Utc>>,
        cx: &mut Context<Self>,
    ) {
        let (tx, rx) = flume::bounded(1);
        self.dispatch(
            Cmd::InstantiateRoutine(id, start_time, tx),
            rx,
            cx,
            |store, actions: Vec<Action>, cx| {
                for action in actions {
                    store.upsert_action_local(action);
                }
                cx.emit(ActionDataChanged);
                cx.emit(DataChanged);
                cx.notify();
            },
        );
    }
}

// ── EventEmitter ──────────────────────────────────────────────────────────────

impl EventEmitter<DatabaseError> for AppDatabaseStore {}
impl EventEmitter<DataChanged> for AppDatabaseStore {}
impl EventEmitter<ActionDataChanged> for AppDatabaseStore {}
impl EventEmitter<EventDataChanged> for AppDatabaseStore {}
impl EventEmitter<RoutineDataChanged> for AppDatabaseStore {}

// ── Global ────────────────────────────────────────────────────────────────────

struct GlobalStore(Entity<AppDatabaseStore>);
impl Global for GlobalStore {}

// ── SSE listener ─────────────────────────────────────────────────────────────
//
// Connects to GET /api/changes/stream and forwards each change notification
// to the GPUI side via `notify_tx`. Reconnects automatically on any error.

async fn sse_loop(client: &reqwest::Client, base: &str, notify_tx: flume::Sender<()>) {
    loop {
        match connect_sse(client, base, &notify_tx).await {
            Ok(()) => {
                tracing::debug!("SSE stream closed by server, reconnecting");
            }
            Err(e) => {
                tracing::warn!(error = %e, "SSE connection failed, retrying in 5s");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
        if notify_tx.is_disconnected() {
            break; // GPUI side is gone — stop reconnecting
        }
    }
}

async fn connect_sse(
    client: &reqwest::Client,
    base: &str,
    notify_tx: &flume::Sender<()>,
) -> Result<(), String> {
    let mut response = client
        .get(format!("{base}/api/changes/stream"))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;

    // Accumulate raw bytes into lines. SSE lines end with '\n'; events are
    // separated by blank lines. We only care about 'data:' lines — and we
    // skip the keep-alive 'ping' texts the server sends every 15 seconds.
    let mut buf = String::new();

    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        buf.push_str(&String::from_utf8_lossy(&chunk));

        // Process every complete line in the buffer.
        while let Some(newline) = buf.find('\n') {
            let line = buf[..newline].trim_end_matches('\r').to_owned();
            buf = buf[newline + 1..].to_owned();

            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if !data.is_empty() && data != "ping" {
                    tracing::debug!(event = data, "SSE change event");
                    if notify_tx.send(()).is_err() {
                        return Ok(()); // GPUI dropped the receiver, stop
                    }
                }
            }
        }
    }

    Ok(())
}

// ── Worker ────────────────────────────────────────────────────────────────────
//
// Runs on a dedicated thread with its own tokio runtime.  Receives Cmd values,
// makes the HTTP request, and sends back the result via the one-shot Reply
// channel embedded in each variant.

async fn run(client: &reqwest::Client, base: &str, cmd: Cmd) {
    match cmd {
        Cmd::FetchAll(tx) => {
            let result = client
                .get(format!("{base}/api/data"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string());
            let result = match result {
                Ok(resp) => resp.json::<AllData>().await.map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            let _ = tx.send(result);
        }

        Cmd::UpsertAction(action, tx) => {
            let result = client
                .put(format!("{base}/api/actions/{}", action.id))
                .json(&action)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map(|_| ())
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        }

        Cmd::DeleteAction(id, tx) => {
            let result = client
                .delete(format!("{base}/api/actions/{id}"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map(|_| ())
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        }

        Cmd::UpsertAndQueueAction(action, tx) => {
            let id = action.id;
            // PUT first — must complete before the server can queue it.
            let put = client
                .put(format!("{base}/api/actions/{id}"))
                .json(&action.clone())
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string());

            let result = match put {
                Err(e) => Err(e),
                Ok(_) if action.is_queued() => {
                    // Parser already assigned a time; don't re-queue.
                    Ok(vec![action])
                }
                Ok(_) => {
                    // Action is backlogged — ask the server to place it in the queue.
                    let r = client
                        .post(format!("{base}/api/actions/{id}/queue"))
                        .send()
                        .await
                        .and_then(|r| r.error_for_status())
                        .map_err(|e| e.to_string());
                    match r {
                        Ok(resp) => resp
                            .json::<Vec<Action>>()
                            .await
                            .map(|v| v)
                            .map_err(|e| e.to_string()),
                        Err(e) => Err(e),
                    }
                }
            };
            let _ = tx.send(result);
        }

        // Cmd::UpsertAndScheduleEvent(event, tx) => {
        //     // Events always have an explicit time; just PUT and return the echo.
        //     let result = client
        //         .put(format!("{base}/api/events/{}", event.id))
        //         .json(&event)
        //         .send()
        //         .await
        //         .and_then(|r| r.error_for_status())
        //         .map_err(|e| e.to_string());
        //     let result = match result {
        //         Ok(_) => Ok(event),
        //         Err(e) => Err(e),
        //     };
        //     let _ = tx.send(result);
        // }
        Cmd::UpsertAndInstantiateRoutine(routine, start_time, tx) => {
            let id = routine.id;
            // PUT the routine first, then instantiate it.
            let put = client
                .put(format!("{base}/api/routines/{id}"))
                .json(&routine)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string());

            let result = match put {
                Err(e) => Err(e),
                Ok(_) => {
                    let body = serde_json::json!({
                        "start_time": start_time,
                    });
                    let r = client
                        .post(format!("{base}/api/routines/{id}/instantiate"))
                        .json(&body)
                        .send()
                        .await
                        .and_then(|r| r.error_for_status())
                        .map_err(|e| e.to_string());
                    match r {
                        Ok(resp) => resp
                            .json::<Vec<Action>>()
                            .await
                            .map(|v| v)
                            .map_err(|e| e.to_string()),
                        Err(e) => Err(e),
                    }
                }
            };
            let _ = tx.send(result);
        }

        Cmd::CompleteAction(id, tx) => {
            let result = client
                .post(format!("{base}/api/actions/{id}/complete"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string());
            let result = match result {
                Ok(resp) => resp
                    .json::<CompleteResult>()
                    .await
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            let _ = tx.send(result);
        }

        Cmd::ClearActionDuration(id, tx) => {
            let result = client
                .post(format!("{base}/api/actions/{id}/clear_duration"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string());
            let result = match result {
                Ok(resp) => resp.json::<Action>().await.map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            let _ = tx.send(result);
        }

        Cmd::QueueAction(id, tx) => {
            let result = client
                .post(format!("{base}/api/actions/{id}/queue"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string());
            let result = match result {
                Ok(resp) => resp
                    .json::<Vec<Action>>()
                    .await
                    .map(|v| v)
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            let _ = tx.send(result);
        }

        Cmd::BacklogAction(id, tx) => {
            let result = client
                .post(format!("{base}/api/actions/{id}/backlog"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string());
            let result = match result {
                Ok(resp) => resp.json::<Action>().await.map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            let _ = tx.send(result);
        }

        Cmd::UpsertEvent(event, tx) => {
            let result = client
                .put(format!("{base}/api/events/{}", event.id))
                .json(&event)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map(|_| ())
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        }

        Cmd::DeleteEvent(id, tx) => {
            let result = client
                .delete(format!("{base}/api/events/{id}"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map(|_| ())
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        }

        Cmd::UpsertRoutine(routine, tx) => {
            let result = client
                .put(format!("{base}/api/routines/{}", routine.id))
                .json(&routine)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map(|_| ())
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        }

        Cmd::DeleteRoutine(id, tx) => {
            let result = client
                .delete(format!("{base}/api/routines/{id}"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map(|_| ())
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        }

        Cmd::InstantiateRoutine(id, start_time, tx) => {
            let body = serde_json::json!({
                "start_time": start_time,
            });
            let result = client
                .post(format!("{base}/api/routines/{id}/instantiate"))
                .json(&body)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string());
            let result = match result {
                Ok(resp) => resp
                    .json::<Vec<Action>>()
                    .await
                    .map(|v| v)
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            let _ = tx.send(result);
        }

        Cmd::RefreshPipeline(tx) => {
            let result = client
                .post(format!("{base}/api/pipeline/refresh"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string());
            let result = match result {
                Ok(resp) => resp
                    .json::<Vec<Action>>()
                    .await
                    .map(|v| v)
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            let _ = tx.send(result);
        }

        Cmd::ExpeditePipeline(tx) => {
            let result = client
                .post(format!("{base}/api/pipeline/expedite"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string());
            let result = match result {
                Ok(resp) => resp
                    .json::<Vec<Action>>()
                    .await
                    .map(|v| v)
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            let _ = tx.send(result);
        }
    }
}
