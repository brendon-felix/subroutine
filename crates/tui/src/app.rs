use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
    debug,
    term::{self, AppTerminal},
    ui::{AppUI, UIAction},
};

#[derive(Debug, Clone)]
pub enum AppAction {
    Tick,
    Render(Instant),
    Resize(u16, u16),
    Quit,
    UIAction(UIAction),
    MultiAction(Vec<AppAction>),
    AfterNTicks(u32, Box<AppAction>),
}

pub struct PendingAction {
    ticks: u32,
    action: AppAction,
}

pub struct App {
    // client: Arc<TickTick>,
    // cached_tasks: Arc<Vec<Arc<Task>>>,
    // pending_tasks: Arc<Mutex<Option<Vec<Task>>>>,
    pending_action: Option<PendingAction>,
    ti: AppTerminal,
    ui: AppUI,
    quitting: bool,
    tx: UnboundedSender<AppAction>,
    rx: UnboundedReceiver<AppAction>,
}

impl App {
    pub fn new() -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        // let cached_tasks = Arc::new(Vec::new());
        // let pending_tasks = Arc::new(Mutex::new(None));
        let pending_action = None;
        let ti = AppTerminal::new()?;
        let ui = AppUI::new(tx.clone());
        let quitting = false;
        debug::init_debug_sender(tx.clone());
        Ok(Self {
            // client,
            // cached_tasks,
            // pending_tasks,
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

    // fn refresh_tasks(&mut self, tx: UnboundedSender<AppAction>) {
    //     // let client = Arc::clone(&self.client);
    //     let pending = Arc::clone(&self.pending_tasks);
    //     tokio::spawn(async move {
    //         match fetch_all_tasks(&client).await {
    //             Ok(tasks) => {
    //                 // Store the tasks in pending storage
    //                 if let Ok(mut guard) = pending.lock() {
    //                     *guard = Some(tasks);
    //                 }
    //                 let _ = tx.send(AppAction::UpdateCache);
    //             }
    //             Err(e) => {
    //                 // let _ = tx.send(AppAction::UIAction(UIAction::DebugMsg(e.to_string())));
    //                 // debug_msg(&e.to_string(), 20, &tx);
    //                 debug!("{}", &e.to_string());
    //             }
    //         }
    //     });
    // }

    // fn update_cache(&mut self) {
    //     let tasks_opt = if let Ok(mut guard) = self.pending_tasks.lock() {
    //         guard.take()
    //     } else {
    //         None
    //     };

    //     if let Some(tasks) = tasks_opt {
    //         self.cached_tasks = Arc::new(tasks.into_iter().map(Arc::new).collect());
    //         self.ui.update_tasks(Arc::clone(&self.cached_tasks));
    //     }
    // }

    fn handle_event(&mut self, event: term::Event, tx: &UnboundedSender<AppAction>) -> Result<()> {
        match event {
            // term::Event::Quit => tx.send(AppAction::Quit)?,
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
        match key_event.code {
            KeyCode::Char('q') if self.ui.allow_key_cmd() => {
                tx.send(AppAction::UIAction(UIAction::Confirm(
                    ratatui::text::Text::from("Are you sure you want to quit?"),
                    Box::new(AppAction::Quit),
                )))?
            }
            // KeyCode::Char('r')
            //     if key_event.modifiers == KeyModifiers::CONTROL && self.ui.allow_key_cmd() =>
            // {
            //     tx.send(AppAction::RefreshData)?
            // }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                tx.send(AppAction::Quit)?
            }
            _ => self.ui.handle_key_event(key_event, tx),
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
                self.ui.reset_areas();
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
        }
        Ok(())
    }

    // fn execute_reschedule_multiple_tasks(
    //     &mut self,
    //     task_data_list: Vec<TaskData>,
    //     tx: &UnboundedSender<AppAction>,
    // ) {
    //     let client = Arc::clone(&self.client);
    //     let _ = tokio::spawn(async move { tasks::reschedule_tasks(&client, task_data_list).await });
    //     self.refresh_tasks(tx.clone())
    // }

    fn render(&mut self, last_frame: Instant) -> Result<()> {
        self.ti.draw(|f| {
            self.ui.draw(f, f.area(), last_frame);
        })?;
        Ok(())
    }

    // fn error(&mut self, _message: String) {}
}
