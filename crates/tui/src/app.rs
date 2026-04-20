use crate::log;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use simple_core::{Action, Event};
use simple_db::{DatabaseConnection, PostgresConfig, connect_and_migrate, sync_once};
use std::time::Instant;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
    debug,
    term::{self, AppTerminal},
    ui::{AppView, RootView, UIAction, utils},
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
            Self::Syncing => write!(f, " \u{f110} syncing…"), // nf-fa-spinner
            Self::Ok => write!(f, " \u{f00c} synced"),        // nf-fa-check
            Self::Error(e) => write!(f, " \u{f071} {e}"),     // nf-fa-warning
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
    UIAction(UIAction),
    MultiAction(Vec<AppAction>),
    AfterNTicks(u32, Box<AppAction>),

    /// Trigger a DB reload into the action cache.
    RefreshActions,
    /// Replace the action cache after a DB reload.
    SetActions(Vec<Action>),
    /// Sync status update from the background sync task.
    SyncStatus(SyncStatus),
    /// Persist a new action created from the input bar.
    CreateAction(String),
    /// Delete (soft-delete) the action by UUID.
    DeleteAction(uuid::Uuid),
    /// Complete the action by UUID.
    CompleteAction(uuid::Uuid),

    /// Trigger a DB reload into the event cache.
    RefreshEvents,
    /// Replace the event cache after a DB reload.
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
    pub db: DatabaseConnection,
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
        let db = connect_and_migrate()?;
        let actions = Vec::new();
        let events = Vec::new();
        let sync_status = SyncStatus::Idle;
        let pending_action = None;
        let ti = AppTerminal::new()?;
        let ui = RootView::new(tx.clone());
        let quitting = false;
        debug::init_debug_sender(tx.clone());
        Ok(Self {
            db,
            actions,
            events,
            sync_status,
            pending_action,
            ti,
            ui,
            quitting,
            tx,
            rx,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let tx = self.tx.clone();
        self.ti.enter()?;

        // Load actions and events on startup.
        tx.send(AppAction::RefreshActions)?;
        tx.send(AppAction::RefreshEvents)?;

        // Kick off a startup sync if Postgres config is available.
        if let Some(pg_config) = postgres_config_from_env() {
            let db = self.db.clone();
            let tx2 = tx.clone();
            let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Syncing));
            tokio::spawn(async move {
                match sync_once(&db, &pg_config).await {
                    Ok(()) => {
                        log!("[sync] startup sync ok");
                        let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Ok));
                        let _ = tx2.send(AppAction::RefreshActions);
                    }
                    Err(e) => {
                        log!("[sync] startup sync error: {e:#}");
                        let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Error(e.to_string())));
                    }
                }
            });
        }

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

        // Sync on quit.
        if let Some(pg_config) = postgres_config_from_env() {
            let db = self.db.clone();
            let _ = sync_once(&db, &pg_config).await;
        }

        self.ti.exit()?;
        Ok(())
    }

    fn handle_event(&mut self, event: term::Event, tx: &UnboundedSender<AppAction>) -> Result<()> {
        match event {
            term::Event::Tick => tx.send(AppAction::Tick)?,
            term::Event::Render(last) => tx.send(AppAction::Render(last))?,
            term::Event::Resize(w, h) => tx.send(AppAction::Resize(w, h))?,
            term::Event::Key(key) => self.handle_key_event(key, tx)?,
            term::Event::Mouse(mouse) => self.handle_mouse_event(mouse, tx)?,
            term::Event::Paste(_content) => {}
            _ => {}
        }
        Ok(())
    }

    fn handle_key_event(
        &mut self,
        key_event: KeyEvent,
        tx: &UnboundedSender<AppAction>,
    ) -> Result<()> {
        if self.ui.handle_key_event(key_event, tx) {
            return Ok(());
        }
        match key_event.code {
            KeyCode::Char('q') => {
                tx.send(AppAction::Quit)?;
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                tx.send(AppAction::Quit)?
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_event(
        &mut self,
        mouse_event: MouseEvent,
        _tx: &UnboundedSender<AppAction>,
    ) -> Result<()> {
        self.ui.handle_mouse_event(mouse_event);
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
            AppAction::UIAction(action) => self.ui.execute_action(action, tx),
            AppAction::MultiAction(actions) => {
                for act in actions {
                    self.execute_action(act, tx)?;
                }
            }
            AppAction::AfterNTicks(n_ticks, action) => {
                self.pending_action = Some(PendingAction {
                    ticks: n_ticks,
                    action: *action,
                });
            }

            AppAction::RefreshActions => {
                let db = self.db.clone();
                let tx2 = tx.clone();
                tokio::spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        let conn = db.lock().unwrap();
                        simple_db::fetch_actions(&conn)
                    })
                    .await;
                    match result {
                        Ok(Ok(actions)) => {
                            let _ = tx2.send(AppAction::SetActions(actions));
                        }
                        Ok(Err(e)) => {
                            let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Error(format!(
                                "DB read error: {e}"
                            ))));
                        }
                        Err(e) => {
                            let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Error(format!(
                                "Task panic: {e}"
                            ))));
                        }
                    }
                });
            }

            AppAction::SetActions(actions) => {
                self.actions = actions.clone();
                self.ui.set_actions(actions);
            }

            AppAction::RefreshEvents => {
                let db = self.db.clone();
                let tx2 = tx.clone();
                tokio::spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        let conn = db.lock().unwrap();
                        simple_db::fetch_events(&conn)
                    })
                    .await;
                    match result {
                        Ok(Ok(events)) => {
                            let _ = tx2.send(AppAction::SetEvents(events));
                        }
                        Ok(Err(e)) => {
                            let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Error(format!(
                                "DB read error: {e}"
                            ))));
                        }
                        Err(_) => {}
                    }
                });
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
                        let db = self.db.clone();
                        let tx2 = tx.clone();
                        tokio::spawn(async move {
                            let result = tokio::task::spawn_blocking(move || {
                                let conn = db.lock().unwrap();
                                simple_db::upsert_event(&conn, &event)
                            })
                            .await;
                            match result {
                                Ok(Ok(())) => {
                                    let _ = tx2.send(AppAction::RefreshEvents);
                                }
                                Ok(Err(e)) => {
                                    log!("[create event] save error: {e:#}");
                                    let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Error(
                                        format!("Save error: {e}"),
                                    )));
                                }
                                Err(_) => {}
                            }
                        });
                    }
                    _ => {
                        log!("[create event] parse/build failed for: {:?}", input);
                    }
                }
            }

            AppAction::DeleteEvent(id) => {
                let db = self.db.clone();
                let tx2 = tx.clone();
                tokio::spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        let conn = db.lock().unwrap();
                        simple_db::delete_event(&conn, id)
                    })
                    .await;
                    match result {
                        Ok(Ok(())) => {
                            let _ = tx2.send(AppAction::RefreshEvents);
                        }
                        Ok(Err(e)) => {
                            let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Error(format!(
                                "Delete error: {e}"
                            ))));
                        }
                        Err(_) => {}
                    }
                });
            }

            AppAction::SyncStatus(status) => {
                self.sync_status = status.clone();
                self.ui.set_sync_status(status);
            }

            AppAction::CreateAction(input) => {
                use simple_parser::{BuildTarget, build_entity, parse_action_input};
                log!("[create] input: {:?}", input);
                let parse_result = parse_action_input(&input);
                log!(
                    "[create] parse result: {:?}",
                    parse_result.as_ref().map(|d| &d.title)
                );
                match parse_result
                    .ok()
                    .and_then(|draft| build_entity(&draft, BuildTarget::Action).ok())
                {
                    Some(simple_parser::BuiltEntity::Action(action)) => {
                        log!("[create] built action: {:?}", action.title);
                        let db = self.db.clone();
                        let tx2 = tx.clone();
                        let action_clone = action.clone();
                        tokio::spawn(async move {
                            let result = tokio::task::spawn_blocking(move || {
                                let conn = db.lock().unwrap();
                                simple_db::upsert_action(&conn, &action_clone)
                            })
                            .await;
                            match result {
                                Ok(Ok(())) => {
                                    log!("[create] saved ok");
                                    let _ = tx2.send(AppAction::RefreshActions);
                                }
                                Ok(Err(e)) => {
                                    log!("[create] save error: {e:#}");
                                    let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Error(
                                        format!("Save error: {e}"),
                                    )));
                                }
                                Err(e) => {
                                    log!("[create] task panic: {e}");
                                    let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Error(
                                        format!("Task panic: {e}"),
                                    )));
                                }
                            }
                        });
                    }
                    _ => {
                        log!("[create] parse/build failed for input: {:?}", input);
                    }
                }
            }

            AppAction::DeleteAction(id) => {
                let db = self.db.clone();
                let tx2 = tx.clone();
                tokio::spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        let conn = db.lock().unwrap();
                        simple_db::delete_action(&conn, id)
                    })
                    .await;
                    match result {
                        Ok(Ok(())) => {
                            let _ = tx2.send(AppAction::RefreshActions);
                        }
                        Ok(Err(e)) => {
                            let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Error(format!(
                                "Delete error: {e}"
                            ))));
                        }
                        Err(_) => {}
                    }
                });
            }

            AppAction::CompleteAction(id) => {
                if let Some(action) = self.actions.iter().find(|a| a.id == id).cloned() {
                    let db = self.db.clone();
                    let tx2 = tx.clone();
                    tokio::spawn(async move {
                        let result = tokio::task::spawn_blocking(move || {
                            let conn = db.lock().unwrap();
                            let mut completed = action.clone();
                            completed.completed_at = Some(chrono::Utc::now());
                            simple_db::upsert_action(&conn, &completed)?;
                            // Remove from pipeline.
                            simple_db::delete_action(&conn, action.id)
                        })
                        .await;
                        match result {
                            Ok(Ok(())) => {
                                let _ = tx2.send(AppAction::RefreshActions);
                            }
                            Ok(Err(e)) => {
                                let _ = tx2.send(AppAction::SyncStatus(SyncStatus::Error(
                                    format!("Complete error: {e}"),
                                )));
                            }
                            Err(_) => {}
                        }
                    });
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, last_frame: Instant) -> Result<()> {
        self.ti.draw(|f| {
            self.ui.draw(f, f.area(), last_frame);
        })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Postgres config from environment / .env
// ---------------------------------------------------------------------------

pub fn postgres_config_from_env() -> Option<PostgresConfig> {
    // Try to load from database.env relative to the workspace root,
    // falling back to real environment variables.
    load_dotenv();

    let host = std::env::var("POSTGRES_HOST").ok()?;
    let port = std::env::var("POSTGRES_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5432);
    let user = std::env::var("POSTGRES_USER").ok()?;
    let password = std::env::var("POSTGRES_PASSWORD").ok()?;
    let dbname = std::env::var("POSTGRES_DBNAME").ok()?;

    Some(PostgresConfig {
        host,
        port,
        user,
        password,
        dbname,
    })
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
