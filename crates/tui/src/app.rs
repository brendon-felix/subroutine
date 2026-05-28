use crate::log;
use crate::store::{DatabaseStore, StoreStatus};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use simple_core::{Action, Event};
use std::time::Instant;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
    debug,
    term::{self, AppTerminal},
    ui::{AppView, RootView},
};

// ---------------------------------------------------------------------------
// Sync status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    Idle,
    Syncing,
    Ok,
    Error(String),
}

impl std::fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, ""),
            Self::Syncing => write!(f, " \u{f110} connecting…"), // nf-fa-spinner
            Self::Ok => write!(f, " \u{f00c} connected"),        // nf-fa-check
            Self::Error(e) => write!(f, " \u{f071} {e}"),        // nf-fa-warning
        }
    }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum AppAction {
    Tick,
    Render(Instant),
    Resize(u16, u16),
    Quit,
    // UIAction(UIAction),
    // MultiAction(Vec<AppAction>),
    // AfterNTicks(u32, Box<AppAction>),
    /// Fired by Store's on_change callback when the initial DB load finishes
    /// (successfully or not). The handler maps StoreStatus → SyncStatus and
    /// kicks off RefreshActions + RefreshEvents.
    StoreReady,

    /// Trigger a reload from the store's in-memory cache into the UI.
    RefreshActions,
    /// Replace the action list held by the UI layer.
    SetActions(Vec<Action>),
    /// Connection / sync status update shown in the status bar.
    SyncStatus(SyncStatus),
    /// Persist a new action created from the input bar.
    CreateAction(String),
    /// Delete (soft-delete) the action by UUID.
    DeleteAction(uuid::Uuid),
    /// Mark the action as complete by UUID.
    CompleteAction(uuid::Uuid),

    /// Trigger a reload from the store's in-memory cache into the UI.
    RefreshEvents,
    /// Replace the event list held by the UI layer.
    SetEvents(Vec<Event>),
    /// Persist a new event created from the input bar.
    CreateEvent(String),
    /// Delete (soft-delete) the event by UUID.
    DeleteEvent(uuid::Uuid),
}

pub struct PendingAction {
    ticks: u32,
    action: AppAction,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    pub store: DatabaseStore,
    pub actions: Vec<Action>,
    pub events: Vec<Event>,
    pub sync_status: SyncStatus,
    pending_action: Option<PendingAction>,
    ti: AppTerminal,
    ui: RootView,
    quitting: bool,
    tx: UnboundedSender<AppAction>,
    rx: UnboundedReceiver<AppAction>,
}

impl App {
    pub fn new() -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();

        // Build the Postgres URL from environment variables, then create the
        // Store. Store::new() returns immediately; the connection and initial
        // data load happen in a background thread. When done (or on error) the
        // on_change closure fires and sends AppAction::StoreReady so we can
        // react in the normal event loop.
        let url = server_url_from_env().unwrap_or_else(|| "http://localhost:3000".to_string());
        let tx_for_store = tx.clone();
        let store = DatabaseStore::new(url, move || {
            let _ = tx_for_store.send(AppAction::StoreReady);
        });

        let ti = AppTerminal::new()?;
        let ui = RootView::new(tx.clone());
        debug::init_debug_sender(tx.clone());

        Ok(Self {
            store,
            actions: Vec::new(),
            events: Vec::new(),
            sync_status: SyncStatus::Syncing,
            pending_action: None,
            ti,
            ui,
            quitting: false,
            tx,
            rx,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let tx = self.tx.clone();
        self.ti.enter()?;

        loop {
            if let Some(event) = self.ti.next().await {
                self.handle_event(event, &tx)?;
            }

            while let Ok(action) = self.rx.try_recv() {
                self.execute_action(action, &tx)?;
            }

            if self.quitting {
                break;
            }
        }

        self.ti.exit()?;
        Ok(())
    }

    fn handle_event(&mut self, event: term::Event, tx: &UnboundedSender<AppAction>) -> Result<()> {
        match event {
            term::Event::Tick => {
                let _ = tx.send(AppAction::Tick);
            }
            term::Event::Render(instant) => {
                let _ = tx.send(AppAction::Render(instant));
            }
            term::Event::Key(key) => self.handle_key_event(key, tx)?,
            term::Event::Mouse(mouse) => self.handle_mouse_event(mouse)?,
            term::Event::Resize(w, h) => {
                let _ = tx.send(AppAction::Resize(w, h));
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent, tx: &UnboundedSender<AppAction>) -> Result<()> {
        if self.ui.handle_key_event(key, tx) {
            return Ok(());
        }
        match key.code {
            KeyCode::Char('q') => {
                let _ = tx.send(AppAction::Quit);
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let _ = tx.send(AppAction::Quit);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<()> {
        self.ui.handle_mouse_event(mouse);
        Ok(())
    }

    fn execute_action(&mut self, action: AppAction, tx: &UnboundedSender<AppAction>) -> Result<()> {
        match action {
            AppAction::Tick => {
                self.ui.next_tick();
                if let Some(pending) = &mut self.pending_action {
                    if pending.ticks > 0 {
                        pending.ticks -= 1;
                    } else {
                        let act = pending.action.clone();
                        self.pending_action = None;
                        self.execute_action(act, tx)?;
                    }
                }
            }
            AppAction::Render(last_frame) => self.render(last_frame)?,
            AppAction::Resize(w, h) => {
                self.ti.resize(w, h)?;
            }
            AppAction::Quit => self.quitting = true,
            // AppAction::UIAction(action) => self.ui.execute_action(action, tx),
            // AppAction::MultiAction(actions) => {
            //     for act in actions {
            //         self.execute_action(act, tx)?;
            //     }
            // }
            // AppAction::AfterNTicks(n_ticks, action) => {
            //     self.pending_action = Some(PendingAction {
            //         ticks: n_ticks,
            //         action: *action,
            //     });
            // }

            // ── Store lifecycle ───────────────────────────────────────────────
            AppAction::StoreReady => {
                // Map StoreStatus → SyncStatus for the status bar.
                let sync = match self.store.status() {
                    StoreStatus::Ready => SyncStatus::Ok,
                    StoreStatus::Error(msg) => SyncStatus::Error(msg),
                    StoreStatus::Loading => SyncStatus::Syncing,
                };
                let _ = tx.send(AppAction::SyncStatus(sync));
                // Populate the UI now that the cache is warm.
                let _ = tx.send(AppAction::RefreshActions);
                let _ = tx.send(AppAction::RefreshEvents);
            }

            // ── Actions ───────────────────────────────────────────────────────
            AppAction::RefreshActions => {
                let actions = self.store.all_actions();
                let _ = tx.send(AppAction::SetActions(actions));
            }

            AppAction::SetActions(actions) => {
                self.actions = actions.clone();
                self.ui.set_actions(actions);
            }

            AppAction::SyncStatus(status) => {
                self.sync_status = status.clone();
                self.ui.set_sync_status(status);
            }

            AppAction::CreateAction(input) => {
                use simple_parser::{BuildTarget, BuiltEntity, build_entity, parse_action_input};
                log!("[create] input: {:?}", input);
                match parse_action_input(&input)
                    .ok()
                    .and_then(|draft| build_entity(&draft, BuildTarget::Action).ok())
                {
                    Some(BuiltEntity::Action(action)) => {
                        log!("[create] built action: {:?}", action.title);
                        self.store.upsert_action(action);
                        let _ = tx.send(AppAction::RefreshActions);
                    }
                    _ => {
                        log!("[create] parse/build failed for: {:?}", input);
                    }
                }
            }

            AppAction::DeleteAction(id) => {
                self.store.trash_action(id);
                let _ = tx.send(AppAction::RefreshActions);
            }

            AppAction::CompleteAction(id) => {
                self.store.complete_action(id);
                let _ = tx.send(AppAction::RefreshActions);
            }

            // ── Events ────────────────────────────────────────────────────────
            AppAction::RefreshEvents => {
                let events = self.store.all_events();
                let _ = tx.send(AppAction::SetEvents(events));
            }

            AppAction::SetEvents(events) => {
                self.events = events.clone();
                self.ui.set_events(events);
            }

            AppAction::CreateEvent(input) => {
                use simple_parser::{BuildTarget, BuiltEntity, build_entity, parse_event_input};
                log!("[create event] input: {:?}", input);
                match parse_event_input(&input)
                    .ok()
                    .and_then(|draft| build_entity(&draft, BuildTarget::Event).ok())
                {
                    Some(BuiltEntity::Event(event)) => {
                        log!("[create event] built: {:?}", event.title);
                        self.store.upsert_event(event);
                        let _ = tx.send(AppAction::RefreshEvents);
                    }
                    _ => {
                        log!("[create event] parse/build failed for: {:?}", input);
                    }
                }
            }

            AppAction::DeleteEvent(id) => {
                self.store.trash_event(id);
                let _ = tx.send(AppAction::RefreshEvents);
            }
        }

        Ok(())
    }

    fn render(&mut self, last_frame: Instant) -> Result<()> {
        self.ti.draw(|f| {
            let area = f.area();
            self.ui.draw(f, area, last_frame);
        })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Database URL from environment / .env file
// ---------------------------------------------------------------------------

pub fn server_url_from_env() -> Option<String> {
    load_dotenv();

    if let Ok(url) = std::env::var("SUBROUTINE_SERVER_URL") {
        return Some(url);
    }

    let host = std::env::var("SUBROUTINE_HOST").ok()?;
    let port: u16 = std::env::var("SUBROUTINE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    Some(format!("http://{host}:{port}"))
}

fn load_dotenv() {
    // Walk up from the binary's location to find database.env.
    // In development cargo puts the binary in target/debug/, so we go up
    // two levels to reach the workspace root.
    let candidates = [
        std::path::PathBuf::from("database.env"),
        std::path::PathBuf::from("../../database.env"),
        std::path::PathBuf::from("../../../database.env"),
    ];
    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, val)) = line.split_once('=') {
                    // Don't override vars already set in the real environment.
                    if std::env::var(key.trim()).is_err() {
                        // SAFETY: single-threaded at startup, before tokio spawns workers.
                        unsafe { std::env::set_var(key.trim(), val.trim()) };
                    }
                }
            }
            break;
        }
    }
}
