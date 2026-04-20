use chrono::{DateTime, Utc};
use gpui::{Context, EventEmitter, Task};
use simple_core::{Action, ActionCompletion, Event, OverlapWarning, Pipeline, QueueItem, Routine};
use simple_db::{
    DatabaseConnection, connect_and_migrate, delete_action, delete_event, delete_routine,
    fetch_actions, fetch_all_completions, fetch_events, fetch_routines, insert_action_completion,
    refresh_pipeline, save_pipeline, upsert_action, upsert_event, upsert_routine,
};
use uuid::Uuid;

macro_rules! lock_db {
    ($conn:expr) => {
        $conn.lock().unwrap_or_else(|poisoned| {
            eprintln!("Warning: database mutex was poisoned, recovering");
            poisoned.into_inner()
        })
    };
}

pub struct DatabaseError {
    pub message: String,
}

pub struct PipelineChanged;
pub struct ActionsLoaded;
pub struct EventsLoaded;
pub struct RoutinesLoaded;
pub struct CompletionsLoaded;

pub struct DatabaseStore {
    conn: Option<DatabaseConnection>,
    pub pipeline: Pipeline,
    pub actions: Vec<Action>,
    pub events: Vec<Event>,
    pub routines: Vec<Routine>,
    pub completions: Vec<ActionCompletion>,
    initialize_task: Option<Task<()>>,
    save_pipeline_task: Option<Task<()>>,
}

/// Runs a blocking database operation on GPUI's background executor, then updates
/// the entity on the foreground thread.
macro_rules! db_op {
    ($self:expr, $cx:expr, $label:expr, $db_work:expr, $on_success:expr) => {{
        let Some(conn) = $self.conn() else {
            return Default::default();
        };
        $cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn({
                    let conn = conn.clone();
                    async move {
                        let connection = lock_db!(conn);
                        ($db_work)(&*connection)
                    }
                })
                .await;
            match result {
                Ok(value) => {
                    if let Err(error) = this.update(cx, |this, cx| {
                        ($on_success)(this, cx, value);
                    }) {
                        eprintln!("Failed to update entity after {}: {error}", $label);
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) = this.update(cx, |_, cx| {
                        Self::emit_error(cx, error_msg);
                    }) {
                        eprintln!("Failed to emit {} error: {update_error}", $label);
                    }
                }
            }
        })
        .detach();
    }};
}

impl DatabaseStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            conn: None,
            pipeline: Pipeline {
                backlog: Vec::new(),
                queue: Vec::new(),
            },
            actions: Vec::new(),
            events: Vec::new(),
            routines: Vec::new(),
            completions: Vec::new(),
            initialize_task: None,
            save_pipeline_task: None,
        }
    }

    fn conn(&self) -> Option<DatabaseConnection> {
        self.conn.clone()
    }

    fn emit_error(cx: &mut Context<Self>, message: String) {
        cx.emit(DatabaseError { message });
    }

    pub fn load_completions(&mut self, cx: &mut Context<Self>) {
        db_op!(
            self,
            cx,
            "load_completions",
            |conn| fetch_all_completions(conn),
            |this: &mut Self, cx: &mut Context<Self>, completions: Vec<ActionCompletion>| {
                this.completions = completions;
                cx.emit(CompletionsLoaded);
            }
        );
    }

    // pub fn get_completions(&self) -> &Vec<ActionCompletion> {
    //     &self.completions
    // }

    // pub fn delete_completion(&mut self, id: Uuid, cx: &mut Context<Self>) {
    //     db_op!(
    //         self,
    //         cx,
    //         "delete_completion",
    //         move |conn| delete_action_completion(conn, id),
    //         |this: &mut Self, cx: &mut Context<Self>, _: ()| {
    //             this.load_completions(cx);
    //         }
    //     );
    // }

    pub fn initialize(&mut self, cx: &mut Context<Self>) {
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { connect_and_migrate() })
                .await;

            match result {
                Ok(conn) => {
                    if let Err(error) = this.update(cx, |this, cx| {
                        this.conn = Some(conn);
                        this.load_saved_actions(cx);
                        this.load_events(cx);
                        this.refresh_pipeline(cx);
                        this.load_routines(cx);
                        this.load_completions(cx);
                    }) {
                        eprintln!("Failed to set connection after initialize: {error}");
                    }
                }
                Err(error) => {
                    if let Err(update_error) = this.update(cx, |_, cx| {
                        Self::emit_error(cx, format!("Database initialization failed: {error}"));
                    }) {
                        eprintln!("Failed to emit initialization error: {update_error}");
                    }
                }
            }
        });
        self.initialize_task = Some(task);
    }

    // ─── Saved Actions ──────────────────────────────────────────────────────────

    pub fn load_saved_actions(&mut self, cx: &mut Context<Self>) {
        db_op!(
            self,
            cx,
            "load_saved_actions",
            |conn| fetch_actions(conn),
            |this: &mut Self, cx: &mut Context<Self>, actions: Vec<Action>| {
                this.actions = actions;
                cx.emit(ActionsLoaded);
            }
        );
    }

    pub fn get_action(&self, id: Uuid) -> Option<&Action> {
        self.actions.iter().find(|a| a.id == id)
    }

    pub fn upsert_action(&mut self, action: Action, cx: &mut Context<Self>) {
        db_op!(
            self,
            cx,
            "upsert_saved_action",
            move |conn| upsert_action(conn, &action),
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.load_saved_actions(cx);
            }
        );
    }

    pub fn delete_action(&mut self, id: Uuid, cx: &mut Context<Self>) {
        db_op!(
            self,
            cx,
            "delete_saved_action",
            move |conn| delete_action(conn, id),
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.load_saved_actions(cx);
            }
        );
    }

    // ─── Saved Events ────────────────────────────────────────────────────────────

    pub fn load_events(&mut self, cx: &mut Context<Self>) {
        db_op!(
            self,
            cx,
            "load_saved_events",
            |conn| fetch_events(conn),
            |this: &mut Self, cx: &mut Context<Self>, events: Vec<Event>| {
                this.events = events;
                cx.emit(EventsLoaded);
            }
        );
    }

    pub fn get_event(&self, id: Uuid) -> Option<&Event> {
        self.events.iter().find(|e| e.id == id)
    }

    pub fn upsert_event(&mut self, event: Event, cx: &mut Context<Self>) {
        db_op!(
            self,
            cx,
            "upsert_saved_event",
            move |conn| upsert_event(conn, &event),
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.load_events(cx);
            }
        );
    }

    pub fn delete_event(&mut self, id: Uuid, cx: &mut Context<Self>) {
        db_op!(
            self,
            cx,
            "delete_saved_event",
            move |conn| delete_event(conn, id),
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.load_events(cx);
            }
        );
    }

    // ─── Pipeline ───────────────────────────────────────────────────────────────

    // pub fn load_pipeline(&mut self, cx: &mut Context<Self>) {
    //     db_op!(
    //         self,
    //         cx,
    //         "load_pipeline",
    //         |conn| load_pipeline(conn),
    //         |this: &mut Self, cx: &mut Context<Self>, pipeline: Pipeline| {
    //             this.pipeline = pipeline;
    //             cx.emit(PipelineChanged);
    //         }
    //     );
    // }

    pub fn refresh_pipeline(&mut self, cx: &mut Context<Self>) {
        db_op!(
            self,
            cx,
            "refresh_pipeline",
            |conn| refresh_pipeline(conn),
            |this: &mut Self, cx: &mut Context<Self>, pipeline: Pipeline| {
                this.pipeline = pipeline;
                cx.emit(PipelineChanged);
            }
        );
    }

    /// Updates an existing action in the queue (matched by ID), preserving
    /// consecutive chains and displacing conflicts. Returns overlap warnings.
    pub fn update_queue_action(
        &mut self,
        action: Action,
        cx: &mut Context<Self>,
    ) -> Vec<OverlapWarning> {
        let warnings = self
            .pipeline
            .update_queue_action(action.clone(), Utc::now());
        let action_for_db = action;
        db_op!(
            self,
            cx,
            "update_queue_action",
            move |conn| {
                upsert_action(conn, &action_for_db)?;
                Ok::<(), anyhow::Error>(())
            },
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.save_pipeline(cx);
                cx.emit(PipelineChanged);
            }
        );
        warnings
    }

    /// Updates an existing event in the queue (matched by ID), preserving
    /// consecutive chains and displacing conflicts. Returns overlap warnings.
    pub fn update_queue_event(
        &mut self,
        event: Event,
        cx: &mut Context<Self>,
    ) -> Vec<OverlapWarning> {
        let warnings = self.pipeline.update_queue_event(event.clone(), Utc::now());
        let event_for_db = event;
        db_op!(
            self,
            cx,
            "update_queue_event",
            move |conn| {
                upsert_event(conn, &event_for_db)?;
                Ok::<(), anyhow::Error>(())
            },
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.save_pipeline(cx);
                cx.emit(PipelineChanged);
            }
        );
        warnings
    }

    pub fn expedite_actions(&mut self, cx: &mut Context<Self>) {
        self.pipeline.expedite_actions(Utc::now());
        self.save_pipeline(cx);
        cx.emit(PipelineChanged);
    }

    pub fn get_queue_action(&self, id: Uuid) -> Option<&Action> {
        self.pipeline.queue.iter().find_map(|item| {
            if let QueueItem::Action(a) = item {
                if a.id == id { Some(a) } else { None }
            } else {
                None
            }
        })
    }

    pub fn get_queue_event(&self, id: Uuid) -> Option<&Event> {
        self.pipeline.queue.iter().find_map(|item| {
            if let QueueItem::Event(e) = item {
                if e.id == id { Some(e) } else { None }
            } else {
                None
            }
        })
    }

    pub fn get_backlog_action(&self, id: Uuid) -> Option<&Action> {
        self.pipeline.backlog.iter().find(|a| a.id == id)
    }

    // /// Adds an action to the backlog and persists the pipeline.
    // pub fn add_action_to_backlog(&mut self, action: Action, cx: &mut Context<Self>) {
    //     let action_for_db = action.clone();
    //     self.pipeline.backlog.push(action);
    //     db_op!(
    //         self,
    //         cx,
    //         "add_action_to_backlog",
    //         move |conn| {
    //             upsert_action(conn, &action_for_db)?;
    //             Ok::<(), anyhow::Error>(())
    //         },
    //         |this: &mut Self, cx: &mut Context<Self>, _: ()| {
    //             this.save_pipeline(cx);
    //             cx.emit(PipelineChanged);
    //         }
    //     );
    // }

    /// Adds an action to the pipeline using smart routing:
    /// - If the action has a `naive_date` (date-only, no time), it goes to the
    ///   backlog. The pipeline's `refresh` will promote it once the date arrives.
    /// - If the action has a full `target` datetime, it is inserted as a static
    ///   action; non-static actions that now conflict are displaced, and overlap
    ///   warnings for immovable items are returned.
    /// - If the action has neither, it is placed in the next available slot
    ///   after consecutive non-static chains and events.
    pub fn add_action_to_queue(
        &mut self,
        action: Action,
        cx: &mut Context<Self>,
    ) -> Vec<OverlapWarning> {
        let id = action.id;
        // Remove any existing copy of this item from both queue and backlog
        // before inserting, so dragging an item that is already present (e.g.
        // a pipeline item dropped back onto the pipeline, or a backlog item
        // dropped onto the pipeline drop-zone) never creates a duplicate.
        self.pipeline.queue.retain(|item| item.id() != id);
        self.pipeline.backlog.retain(|a| a.id != id);

        let warnings = if action.naive_date.is_some() {
            // Date-only: park in the backlog; refresh will promote it.
            self.pipeline.backlog.push(action.clone());
            Vec::new()
        } else if action.target.is_some() {
            self.pipeline
                .queue_action_static(action.clone(), Utc::now())
        } else {
            self.pipeline.queue_action_auto(action.clone(), Utc::now());
            Vec::new()
        };

        let action_for_db = action;
        db_op!(
            self,
            cx,
            "add_action_to_queue",
            move |conn| {
                upsert_action(conn, &action_for_db)?;
                Ok::<(), anyhow::Error>(())
            },
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.save_pipeline(cx);
                cx.emit(PipelineChanged);
            }
        );

        warnings
    }

    /// Adds an event to the queue, displaces any non-static actions that
    /// conflict with it, and returns overlap warnings for immovable items.
    pub fn add_event_to_queue(
        &mut self,
        event: Event,
        cx: &mut Context<Self>,
    ) -> Vec<OverlapWarning> {
        let id = event.id;
        // Remove any existing copy of this event from the queue before
        // inserting, so dropping a pipeline event back onto the pipeline
        // drop-zone never creates a duplicate.
        self.pipeline.queue.retain(|item| item.id() != id);

        let warnings = self.pipeline.queue_event(event.clone(), Utc::now());

        let event_for_db = event;
        db_op!(
            self,
            cx,
            "add_event_to_queue",
            move |conn| {
                upsert_event(conn, &event_for_db)?;
                Ok::<(), anyhow::Error>(())
            },
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.save_pipeline(cx);
                cx.emit(PipelineChanged);
            }
        );

        warnings
    }

    // /// Creates a new ephemeral action and places it in the next available
    // /// slot in the queue.
    // pub fn create_action(&mut self, title: String, cx: &mut Context<Self>) {
    //     let action = Action::new(title);
    //     self.add_action_to_queue(action, cx);
    // }

    /// Promotes a backlog action to the queue, scheduling it in the next
    /// available slot after now (respecting consecutive actions and events).
    pub fn promote_action(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if !self.pipeline.promote_action(id, Utc::now()) {
            tracing::warn!("promote: action {id} not found in backlog");
            return;
        }
        self.save_pipeline(cx);
        cx.emit(PipelineChanged);
    }

    /// Demotes a queue action back to the backlog.
    pub fn demote_action(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let pos = self.pipeline.queue.iter().position(|item| item.id() == id);
        let Some(pos) = pos else {
            tracing::warn!("demote: item {id} not found in queue");
            return;
        };
        let item = self.pipeline.queue.remove(pos);
        if let QueueItem::Action(mut action) = item {
            action.target = None;
            action.target_static = false;
            self.pipeline.backlog.push(action);
        }
        self.save_pipeline(cx);
        cx.emit(PipelineChanged);
    }

    /// Removes an item from whichever list it is in without deleting the underlying record.
    pub fn remove_from_pipeline(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.pipeline.queue.retain(|item| item.id() != id);
        self.pipeline.backlog.retain(|a| a.id != id);
        self.save_pipeline(cx);
        cx.emit(PipelineChanged);
    }

    /// Removes an action from the pipeline and permanently deletes its DB record.
    pub fn delete_queue_action(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.pipeline.queue.retain(|item| item.id() != id);
        self.pipeline.backlog.retain(|a| a.id != id);
        db_op!(
            self,
            cx,
            "delete_queue_action",
            move |conn| delete_action(conn, id),
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.save_pipeline(cx);
                this.load_saved_actions(cx);
                cx.emit(PipelineChanged);
            }
        );
    }

    /// Completes a queue action: records a completion, removes from pipeline,
    /// and if the action has recurrence schedules the next instance.
    pub fn complete_action(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let pos = self.pipeline.queue.iter().position(|item| item.id() == id);
        let Some(pos) = pos else {
            tracing::warn!("complete_action: action {id} not found in queue");
            return;
        };

        let item = self.pipeline.queue.remove(pos);
        let QueueItem::Action(action) = item else {
            tracing::warn!("complete_action: item {id} is not an action");
            return;
        };

        let completion = ActionCompletion::new(&action);
        let mut action = action;
        action.completed_at = Some(completion.completed_at);
        let next = action.next_recurrence();

        if let Some(next_action) = next.clone() {
            self.pipeline
                .queue
                .push(QueueItem::Action(next_action.clone()));
            self.pipeline
                .queue
                .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));
        }

        db_op!(
            self,
            cx,
            "complete_action",
            move |conn| {
                upsert_action(conn, &action)?;
                insert_action_completion(conn, &completion)?;
                if let Some(next_action) = &next {
                    upsert_action(conn, next_action)?;
                }
                Ok::<(), anyhow::Error>(())
            },
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.save_pipeline(cx);
                this.load_completions(cx);
                cx.emit(PipelineChanged);
            }
        );
    }

    /// Instantiates all steps of a routine as ephemeral actions and adds them to the queue.
    pub fn instantiate_routine(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let routine = match self.routines.iter().find(|r| r.id == id) {
            Some(r) => r.clone(),
            None => {
                tracing::warn!("run_routine: no routine found with id {id}");
                return;
            }
        };

        let now = Utc::now();
        let actions: Vec<Action> = routine
            .steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                let offset = routine
                    .steps
                    .iter()
                    .take(i)
                    .fold(chrono::Duration::zero(), |acc, s| {
                        acc + s.duration.unwrap_or(chrono::Duration::zero())
                    });
                Action::new(step.title.clone())
                    .with_target(now + offset, true)
                    .with_origin_routine(routine.id)
                    .with_duration(step.duration.unwrap_or(chrono::Duration::minutes(15)))
            })
            .collect();

        for action in &actions {
            self.pipeline.queue.push(QueueItem::Action(action.clone()));
        }
        self.pipeline
            .queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));

        let actions_for_db = actions.clone();
        db_op!(
            self,
            cx,
            "run_routine",
            move |conn| {
                for action in &actions_for_db {
                    upsert_action(conn, action)?;
                }
                Ok::<(), anyhow::Error>(())
            },
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.save_pipeline(cx);
                cx.emit(PipelineChanged);
            }
        );
    }

    fn save_pipeline(&mut self, cx: &mut Context<Self>) {
        let pipeline = self.pipeline.clone();
        let Some(conn) = self.conn() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let connection = lock_db!(conn);
            if let Err(error) = save_pipeline(&connection, &pipeline) {
                tracing::error!("Failed to save pipeline: {error}");
            }
        });
        self.save_pipeline_task = Some(task);
    }

    // ─── Routines ────────────────────────────────────────────────────────────────

    pub fn load_routines(&mut self, cx: &mut Context<Self>) {
        db_op!(
            self,
            cx,
            "load_routines",
            |conn| fetch_routines(conn),
            |this: &mut Self, cx: &mut Context<Self>, routines: Vec<Routine>| {
                this.routines = routines;
                cx.emit(RoutinesLoaded);
            }
        );
    }

    pub fn get_routine(&self, id: Uuid) -> Option<&Routine> {
        self.routines.iter().find(|r| r.id == id)
    }

    pub fn upsert_routine(&mut self, routine: Routine, cx: &mut Context<Self>) {
        db_op!(
            self,
            cx,
            "upsert_routine",
            move |conn| upsert_routine(conn, &routine),
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.load_routines(cx);
            }
        );
    }

    pub fn delete_routine(&mut self, id: Uuid, cx: &mut Context<Self>) {
        db_op!(
            self,
            cx,
            "delete_routine",
            move |conn| delete_routine(conn, id),
            |this: &mut Self, cx: &mut Context<Self>, _: ()| {
                this.load_routines(cx);
            }
        );
    }
}

impl EventEmitter<DatabaseError> for DatabaseStore {}
impl EventEmitter<PipelineChanged> for DatabaseStore {}
impl EventEmitter<ActionsLoaded> for DatabaseStore {}
impl EventEmitter<EventsLoaded> for DatabaseStore {}
impl EventEmitter<RoutinesLoaded> for DatabaseStore {}
impl EventEmitter<CompletionsLoaded> for DatabaseStore {}
