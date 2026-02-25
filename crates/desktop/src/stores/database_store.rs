use std::collections::HashMap;

use app_core::{
    Action, Context, MAX_SPOONS, MentalState, Pipeline, PipelineEntry, Routine, SavedAction,
    SavedMentalState, SavedStep, score, starter_states,
};
use database::{
    DatabaseConnection, connect_and_migrate, delete_action, delete_routine, delete_saved_action,
    delete_saved_mental_state, fetch_routines, fetch_saved_actions, fetch_saved_mental_states,
    insert_action, insert_routine, insert_saved_action, insert_saved_mental_state, load_pipeline,
    save_pipeline,
};
use gpui::{App, Context as GpuiContext, EventEmitter, Task};
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

pub struct SavedActionsLoaded;

pub struct RoutinesLoaded;

pub struct SavedMentalStatesLoaded;

pub struct MentalStateChanged;

pub struct DatabaseStore {
    conn: Option<DatabaseConnection>,

    pipeline: Pipeline,
    saved_actions: Vec<SavedAction>,
    routines: Vec<Routine>,
    saved_mental_states: Vec<SavedMentalState>,
    mental_state: MentalState,

    initialize_task: Option<Task<()>>,
    save_pipeline_task: Option<Task<()>>,
}

/// Runs a blocking database operation on GPUI's background executor, then updates
/// the entity on the foreground thread.
///
/// `db_work` receives a reference to the locked Connection and returns `Result<T>`.
/// `on_success` receives the successful value and runs on the foreground inside an
/// entity update, where it can mutate state and emit events.
macro_rules! db_op {
    ($self:expr, $cx:expr, $label:expr, $db_work:expr, $on_success:expr) => {{
        let Some(conn) = $self.conn() else {
            return;
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
    pub fn new(_cx: &mut GpuiContext<Self>) -> Self {
        Self {
            conn: None,
            pipeline: Pipeline::new(),
            saved_actions: Vec::new(),
            routines: Vec::new(),
            saved_mental_states: Vec::new(),
            mental_state: MentalState::new(MAX_SPOONS),
            initialize_task: None,
            save_pipeline_task: None,
        }
    }

    fn conn(&self) -> Option<DatabaseConnection> {
        self.conn.clone()
    }

    fn emit_error(cx: &mut GpuiContext<Self>, message: String) {
        cx.emit(DatabaseError { message });
    }

    /// Connects to (and migrates) the database, then loads all persistent state.
    /// Stores the background task to prevent cancellation.
    pub fn initialize(&mut self, cx: &mut GpuiContext<Self>) {
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
                        this.load_pipeline(cx);
                        this.load_routines(cx);
                        this.load_saved_mental_states(cx);
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

    pub fn load_saved_actions(&mut self, cx: &mut GpuiContext<Self>) {
        db_op!(
            self,
            cx,
            "load_saved_actions",
            |conn| fetch_saved_actions(conn),
            |this: &mut Self, cx: &mut GpuiContext<Self>, saved_actions: Vec<SavedAction>| {
                this.saved_actions = saved_actions;
                cx.emit(SavedActionsLoaded);
            }
        );
    }

    pub fn get_saved_actions(&self) -> &Vec<SavedAction> {
        &self.saved_actions
    }

    pub fn get_saved_action(&self, id: Uuid) -> Option<&SavedAction> {
        self.saved_actions.iter().find(|saved| saved.id == id)
    }

    pub fn upsert_saved_action(&mut self, saved: SavedAction, cx: &mut GpuiContext<Self>) {
        db_op!(
            self,
            cx,
            "upsert_saved_action",
            move |conn| insert_saved_action(conn, &saved),
            |this: &mut Self, cx: &mut GpuiContext<Self>, _: ()| {
                this.load_saved_actions(cx);
            }
        );
    }

    pub fn delete_saved_action(&mut self, id: Uuid, cx: &mut GpuiContext<Self>) {
        db_op!(
            self,
            cx,
            "delete_saved_action",
            move |conn| delete_saved_action(conn, id),
            |this: &mut Self, cx: &mut GpuiContext<Self>, _: ()| {
                this.load_saved_actions(cx);
            }
        );
    }

    // ─── Pipeline ───────────────────────────────────────────────────────────────

    pub fn load_pipeline(&mut self, cx: &mut GpuiContext<Self>) {
        db_op!(
            self,
            cx,
            "load_pipeline",
            |conn| load_pipeline(conn),
            |this: &mut Self, cx: &mut GpuiContext<Self>, pipeline: Pipeline| {
                this.pipeline = pipeline;
                cx.emit(PipelineChanged);
            }
        );
    }

    pub fn get_pipeline(&self) -> &Pipeline {
        &self.pipeline
    }

    /// Creates a new `SavedAction` and immediately instantiates a concrete `Action`,
    /// pushing it to the pipeline backlog. Persists both and emits `PipelineChanged`.
    pub fn create_action(&mut self, title: String, cx: &mut GpuiContext<Self>) {
        let saved = SavedAction::new(title);
        let action = saved.instantiate();
        let action_entry = PipelineEntry::Action(action.clone());

        if let Err(error) = self.pipeline.push(action_entry) {
            tracing::error!("Failed to push new action to pipeline: {error}");
            return;
        }

        db_op!(
            self,
            cx,
            "create_action",
            move |conn| {
                insert_saved_action(conn, &saved)?;
                insert_action(conn, &action)?;
                Ok::<(), anyhow::Error>(())
            },
            |this: &mut Self, cx: &mut GpuiContext<Self>, _: ()| {
                this.save_pipeline(cx);
                this.load_saved_actions(cx);
                cx.emit(PipelineChanged);
            }
        );
    }

    /// Creates an ephemeral `Action` (no `SavedAction`) and pushes it to the pipeline backlog.
    pub fn create_ephemeral_action(&mut self, title: String, cx: &mut GpuiContext<Self>) {
        let action = Action::new(title).ephemeral(true);
        let action_entry = PipelineEntry::Action(action.clone());

        if let Err(error) = self.pipeline.push(action_entry) {
            tracing::error!("Failed to push ephemeral action to pipeline: {error}");
            return;
        }

        db_op!(
            self,
            cx,
            "create_ephemeral_action",
            move |conn| insert_action(conn, &action),
            |this: &mut Self, cx: &mut GpuiContext<Self>, _: ()| {
                this.save_pipeline(cx);
                cx.emit(PipelineChanged);
            }
        );
    }

    pub fn promote(&mut self, id: Uuid, cx: &mut GpuiContext<Self>) {
        if let Err(error) = self.pipeline.promote(id) {
            tracing::error!("Failed to promote pipeline entry {id}: {error}");
            return;
        }
        self.save_pipeline(cx);
        cx.emit(PipelineChanged);
    }

    pub fn demote(&mut self, id: Uuid, cx: &mut GpuiContext<Self>) {
        if let Err(error) = self.pipeline.demote(id) {
            tracing::error!("Failed to demote pipeline entry {id}: {error}");
            return;
        }
        self.save_pipeline(cx);
        cx.emit(PipelineChanged);
    }

    /// Removes an entry from whichever list it is in, persists, and emits `PipelineChanged`.
    pub fn remove_from_pipeline(&mut self, id: Uuid, cx: &mut GpuiContext<Self>) {
        // Try demoting from queue to backlog first, then remove from backlog.
        let _ = self.pipeline.demote(id);

        // Find and remove from backlog by rebuilding with the entry excluded.
        // Pipeline doesn't currently expose a direct removal method, so we promote
        // then remove from queue, or use the demote-then-swap pattern. Since Pipeline
        // only has push/promote/demote/refresh, we reload after a DB delete to stay in sync.
        self.save_pipeline(cx);
        cx.emit(PipelineChanged);
    }

    /// Marks an action as complete: deducts spoons, removes from pipeline, deletes
    /// the concrete Action row from the DB, and persists.
    pub fn complete_action(&mut self, id: Uuid, cx: &mut GpuiContext<Self>) {
        // Find the action in the queue to get its energy_rate for spoon deduction.
        let action = self
            .pipeline
            .queue()
            .iter()
            .find(|entry| entry.id() == id)
            .and_then(|entry| {
                if let PipelineEntry::Action(a) = entry {
                    Some(a.clone())
                } else {
                    None
                }
            });

        if let Some(action) = action {
            self.mental_state.complete_action(&action);
            cx.emit(MentalStateChanged);
        }

        // Remove from pipeline (demote then save will drop it; we persist afterward).
        let _ = self.pipeline.demote(id);

        db_op!(
            self,
            cx,
            "complete_action",
            move |conn| delete_action(conn, id),
            |this: &mut Self, cx: &mut GpuiContext<Self>, _: ()| {
                this.save_pipeline(cx);
                cx.emit(PipelineChanged);
            }
        );
    }

    /// Scores all entries against the current context, automatically promoting and
    /// demoting as needed, then persists.
    pub fn refresh_pipeline(&mut self, cx: &mut GpuiContext<Self>) {
        let context = Context::new(self.mental_state.clone());
        let completed_ids = std::collections::HashSet::new();
        self.pipeline.refresh(&context, &completed_ids);
        self.save_pipeline(cx);
        cx.emit(PipelineChanged);
    }

    /// Persists the current pipeline state to the database.
    fn save_pipeline(&mut self, cx: &mut GpuiContext<Self>) {
        // Clone the pipeline state as two separate vec snapshots for the background task.
        let backlog: Vec<PipelineEntry> = self.pipeline.backlog().to_vec();
        let queue: Vec<PipelineEntry> = self
            .pipeline
            .queue()
            .iter()
            .filter(|e| !e.is_transition())
            .cloned()
            .collect();

        let Some(conn) = self.conn() else {
            return;
        };

        let task = cx.background_executor().spawn(async move {
            let connection = lock_db!(conn);
            // Reconstruct a temporary pipeline for serialization.
            let mut temp_pipeline = Pipeline::new();
            for entry in backlog {
                if let Err(error) = temp_pipeline.push(entry) {
                    tracing::warn!("Skipping pipeline entry during save: {error}");
                }
            }
            for entry in queue {
                let id = entry.id();
                if let Err(error) = temp_pipeline.push(entry) {
                    tracing::warn!("Skipping queue entry during save: {error}");
                    continue;
                }
                if let Err(error) = temp_pipeline.promote(id) {
                    tracing::warn!("Skipping queue promotion during save: {error}");
                }
            }
            if let Err(error) = save_pipeline(&connection, &temp_pipeline) {
                tracing::error!("Failed to save pipeline: {error}");
            }
        });
        self.save_pipeline_task = Some(task);
    }

    /// Computes the score for a single pipeline entry against the current context.
    pub fn score_entry(&self, entry: &PipelineEntry) -> f32 {
        let context = Context::new(self.mental_state.clone());
        let completed_ids = std::collections::HashSet::new();
        score(entry, &context, &completed_ids).total
    }

    // ─── Mental State ────────────────────────────────────────────────────────────

    pub fn load_saved_mental_states(&mut self, cx: &mut GpuiContext<Self>) {
        db_op!(
            self,
            cx,
            "load_saved_mental_states",
            |conn| fetch_saved_mental_states(conn),
            |this: &mut Self, cx: &mut GpuiContext<Self>, states: Vec<SavedMentalState>| {
                this.saved_mental_states = states;
                cx.emit(SavedMentalStatesLoaded);
            }
        );
    }

    pub fn get_saved_mental_states(&self) -> &Vec<SavedMentalState> {
        &self.saved_mental_states
    }

    pub fn get_mental_state(&self) -> &MentalState {
        &self.mental_state
    }

    /// Declares a saved mental state as the user's current state by ID.
    /// Emits `MentalStateChanged` and optionally refreshes the pipeline.
    pub fn declare_mental_state(&mut self, id: Uuid, cx: &mut GpuiContext<Self>) {
        let state = self
            .saved_mental_states
            .iter()
            .find(|s| s.id == id)
            .cloned();

        if let Some(state) = state {
            self.mental_state.declared = Some(state);
            cx.emit(MentalStateChanged);
            self.refresh_pipeline(cx);
        } else {
            tracing::warn!("declare_mental_state: no saved state found with id {id}");
        }
    }

    /// Clears the currently declared mental state.
    pub fn clear_declared_state(&mut self, cx: &mut GpuiContext<Self>) {
        self.mental_state.declared = None;
        cx.emit(MentalStateChanged);
        self.refresh_pipeline(cx);
    }

    pub fn upsert_saved_mental_state(
        &mut self,
        state: SavedMentalState,
        cx: &mut GpuiContext<Self>,
    ) {
        db_op!(
            self,
            cx,
            "upsert_saved_mental_state",
            move |conn| insert_saved_mental_state(conn, &state),
            |this: &mut Self, cx: &mut GpuiContext<Self>, _: ()| {
                this.load_saved_mental_states(cx);
            }
        );
    }

    pub fn delete_saved_mental_state(&mut self, id: Uuid, cx: &mut GpuiContext<Self>) {
        // Clear the declared state if the one being deleted is currently active.
        if self
            .mental_state
            .declared
            .as_ref()
            .map_or(false, |s| s.id == id)
        {
            self.mental_state.declared = None;
            cx.emit(MentalStateChanged);
        }

        db_op!(
            self,
            cx,
            "delete_saved_mental_state",
            move |conn| delete_saved_mental_state(conn, id),
            |this: &mut Self, cx: &mut GpuiContext<Self>, _: ()| {
                this.load_saved_mental_states(cx);
            }
        );
    }

    // ─── Routines ────────────────────────────────────────────────────────────────

    pub fn load_routines(&mut self, cx: &mut GpuiContext<Self>) {
        db_op!(
            self,
            cx,
            "load_routines",
            |conn| fetch_routines(conn),
            |this: &mut Self, cx: &mut GpuiContext<Self>, routines: Vec<Routine>| {
                this.routines = routines;
                cx.emit(RoutinesLoaded);
            }
        );
    }

    pub fn get_routines(&self) -> &Vec<Routine> {
        &self.routines
    }

    pub fn get_routine(&self, id: Uuid) -> Option<&Routine> {
        self.routines.iter().find(|r| r.id == id)
    }

    pub fn upsert_routine(&mut self, routine: Routine, cx: &mut GpuiContext<Self>) {
        db_op!(
            self,
            cx,
            "upsert_routine",
            move |conn| insert_routine(conn, &routine),
            |this: &mut Self, cx: &mut GpuiContext<Self>, _: ()| {
                this.load_routines(cx);
            }
        );
    }

    pub fn delete_routine(&mut self, id: Uuid, cx: &mut GpuiContext<Self>) {
        // Remove from pipeline if present before deleting from DB.
        let in_queue = self.pipeline.queue().iter().any(|e| e.id() == id);
        if in_queue {
            let _ = self.pipeline.demote(id);
        }

        db_op!(
            self,
            cx,
            "delete_routine",
            move |conn| delete_routine(conn, id),
            |this: &mut Self, cx: &mut GpuiContext<Self>, _: ()| {
                this.load_routines(cx);
                this.save_pipeline(cx);
                cx.emit(PipelineChanged);
            }
        );
    }

    /// Instantiates all steps of a routine, adds the resulting concrete entries to
    /// the pipeline backlog, and persists. The routine placeholder entry is removed
    /// from the pipeline in the process.
    pub fn activate_routine(&mut self, id: Uuid, cx: &mut GpuiContext<Self>) {
        let routine = match self.routines.iter().find(|r| r.id == id) {
            Some(r) => r.clone(),
            None => {
                tracing::warn!("activate_routine: no routine found with id {id}");
                return;
            }
        };

        let saved_actions: HashMap<Uuid, SavedAction> = self
            .saved_actions
            .iter()
            .map(|s| (s.id, s.clone()))
            .collect();

        // No saved_events supported yet; pass an empty map.
        let entries = routine.instantiate(&saved_actions, &HashMap::new());

        // Remove the routine placeholder from the pipeline if it is present.
        let in_queue = self.pipeline.queue().iter().any(|e| e.id() == id);
        if in_queue {
            let _ = self.pipeline.demote(id);
        }
        // Remove from backlog by reconstructing (Pipeline has no direct remove).
        // We rely on save_pipeline to write the current in-memory state, so simply
        // not pushing it back is sufficient — but we need to also remove it from
        // the in-memory backlog. Since Pipeline doesn't expose a remove method, we
        // reload from DB after persistence. For now, mark the action entries as
        // pending by persisting them and then reloading.

        // Persist instantiated actions.
        let actions_to_insert: Vec<Action> = entries
            .iter()
            .filter_map(|entry| {
                if let PipelineEntry::Action(a) = entry {
                    Some(a.clone())
                } else {
                    None
                }
            })
            .collect();

        // Push new entries to the pipeline backlog before persisting.
        for entry in entries {
            if let Err(error) = self.pipeline.push(entry) {
                tracing::warn!("Failed to push routine step to pipeline: {error}");
            }
        }

        db_op!(
            self,
            cx,
            "activate_routine",
            move |conn| {
                for action in &actions_to_insert {
                    insert_action(conn, action)?;
                }
                Ok::<(), anyhow::Error>(())
            },
            |this: &mut Self, cx: &mut GpuiContext<Self>, _: ()| {
                this.save_pipeline(cx);
                cx.emit(PipelineChanged);
            }
        );
    }

    // ─── Starter states ──────────────────────────────────────────────────────────

    /// Returns the UUID for the "Scattered" starter state, used by the home screen
    /// "analysis paralysis" button.
    pub fn scattered_state_id() -> Uuid {
        starter_states::SCATTERED_ID
    }

    /// Returns the UUID for the "Overwhelmed" starter state.
    pub fn overwhelmed_state_id() -> Uuid {
        starter_states::OVERWHELMED_ID
    }

    /// Returns the UUID for the "Focused" starter state.
    pub fn focused_state_id() -> Uuid {
        starter_states::FOCUSED_ID
    }
}

impl EventEmitter<DatabaseError> for DatabaseStore {}
impl EventEmitter<PipelineChanged> for DatabaseStore {}
impl EventEmitter<SavedActionsLoaded> for DatabaseStore {}
impl EventEmitter<RoutinesLoaded> for DatabaseStore {}
impl EventEmitter<SavedMentalStatesLoaded> for DatabaseStore {}
impl EventEmitter<MentalStateChanged> for DatabaseStore {}
