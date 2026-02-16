use gpui::{Context, EventEmitter};

use database::{
    self, Action, ContextSnapshot, DEFAULT_PIPELINE_ID, DatabaseConnection, EventType, Instance,
    InstantiateRoutineOptions, MentalState, MentalStateEvent, PipelineItem, Routine, RoutineStep,
    ScoredInstance,
};

pub struct DatabaseError {
    pub message: String,
}

pub struct ActionsLoaded;

pub struct PipelineLoaded;

pub struct ContextLoaded;

pub struct MentalStatesLoaded;

pub struct PipelineScored;

pub struct SuggestionsLoaded;

pub struct RoutinesLoaded;

pub struct RoutineStepsLoaded {
    pub routine_id: String,
}

pub struct DatabaseStore {
    conn: Option<DatabaseConnection>,
    actions: Vec<Action>,
    instances: Vec<Instance>,
    pipeline_items: Vec<PipelineItem>,
    current_context: Option<ContextSnapshot>,
    mental_states: Vec<MentalState>,
    current_mental_state: Option<MentalState>,
    pipeline_scores: Vec<(String, f64)>,
    suggestions: Vec<(Instance, Action, f64)>,
    routines: Vec<Routine>,
    routine_steps: Vec<RoutineStep>,
}

impl DatabaseStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            conn: None,
            actions: Vec::new(),
            instances: Vec::new(),
            pipeline_items: Vec::new(),
            current_context: None,
            mental_states: Vec::new(),
            current_mental_state: None,
            pipeline_scores: Vec::new(),
            suggestions: Vec::new(),
            routines: Vec::new(),
            routine_steps: Vec::new(),
        }
    }

    fn conn(&self) -> Option<DatabaseConnection> {
        self.conn.as_ref().cloned()
    }

    fn emit_error(cx: &mut Context<Self>, message: impl Into<String>) {
        cx.emit(DatabaseError {
            message: message.into(),
        });
        cx.notify();
    }

    pub fn initialize(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(|| {
                let conn = database::connect_and_migrate()?;
                {
                    let connection = conn.lock().unwrap();
                    database::ensure_default_pipeline(&connection)?;
                }
                Ok::<_, anyhow::Error>(conn)
            })
            .await;

            match result {
                Ok(Ok(conn)) => {
                    if let Err(error) = this.update(cx, |this, cx| {
                        this.conn = Some(conn);
                        this.load_all_actions(cx);
                        this.load_all_instances(cx);
                        this.load_pipeline(cx);
                        this.load_current_context(cx);
                        this.load_mental_states(cx);
                        this.load_current_mental_state(cx);
                        this.load_routines(cx);
                        cx.notify();
                    }) {
                        eprintln!("Failed to store database connection: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit initialization error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit initialization error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn insert_action(&self, action: Action, cx: &Context<Self>) {
        if let Some(conn) = self.conn() {
            let action_title = action.title.clone();
            cx.spawn(async move |this, cx| {
                let result = tokio::task::spawn_blocking(move || {
                    let connection = conn.lock().unwrap();
                    database::insert_action(&connection, &action)
                })
                .await;

                match result {
                    Ok(Ok(())) => {
                        if let Err(error) = this.update(cx, |this, cx| {
                            this.load_all_actions(cx);
                            println!("Inserted action {}\n", action_title);
                            cx.notify();
                        }) {
                            eprintln!("Failed to refresh actions after insert: {error}");
                        }
                    }
                    Ok(Err(error)) => {
                        let error_msg = format!("{error}");
                        if let Err(update_error) =
                            this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                        {
                            eprintln!("Failed to emit action insert error: {update_error}");
                        }
                    }
                    Err(error) => {
                        let error_msg = format!("{error}");
                        if let Err(update_error) =
                            this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                        {
                            eprintln!("Failed to emit action insert error: {update_error}");
                        }
                    }
                }
            })
            .detach();
        }
    }

    pub fn load_all_actions(&self, cx: &Context<Self>) {
        if let Some(conn) = self.conn() {
            cx.spawn(async move |this, cx| {
                let result = tokio::task::spawn_blocking(move || {
                    let connection = conn.lock().unwrap();
                    database::fetch_actions(&connection)
                })
                .await;

                match result {
                    Ok(Ok(actions)) => {
                        if let Err(error) = this.update(cx, move |this, cx| {
                            this.actions = actions;
                            cx.emit(ActionsLoaded);
                            cx.notify();
                        }) {
                            eprintln!("Failed to update actions: {error}");
                        }
                    }
                    Ok(Err(error)) => {
                        let error_msg = format!("{error}");
                        if let Err(update_error) =
                            this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                        {
                            eprintln!("Failed to emit action load error: {update_error}");
                        }
                    }
                    Err(error) => {
                        let error_msg = format!("{error}");
                        if let Err(update_error) =
                            this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                        {
                            eprintln!("Failed to emit action load error: {update_error}");
                        }
                    }
                }
            })
            .detach();
        }
    }

    pub fn get_all_actions(&self) -> &Vec<Action> {
        &self.actions
    }

    pub fn get_action(&self, action_id: &str) -> Option<&Action> {
        self.actions.iter().find(|action| action.id == action_id)
    }

    pub fn load_all_instances(&self, cx: &Context<Self>) {
        if let Some(conn) = self.conn() {
            cx.spawn(async move |this, cx| {
                let result = tokio::task::spawn_blocking(move || {
                    let connection = conn.lock().unwrap();
                    database::fetch_instances(&connection)
                })
                .await;

                match result {
                    Ok(Ok(instances)) => {
                        if let Err(error) = this.update(cx, move |this, cx| {
                            this.instances = instances;
                            cx.notify();
                        }) {
                            eprintln!("Failed to update instances: {error}");
                        }
                    }
                    Ok(Err(error)) => {
                        let error_msg = format!("{error}");
                        if let Err(update_error) =
                            this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                        {
                            eprintln!("Failed to emit instance load error: {update_error}");
                        }
                    }
                    Err(error) => {
                        let error_msg = format!("{error}");
                        if let Err(update_error) =
                            this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                        {
                            eprintln!("Failed to emit instance load error: {update_error}");
                        }
                    }
                }
            })
            .detach();
        }
    }

    pub fn get_instance(&self, instance_id: &str) -> Option<&Instance> {
        self.instances
            .iter()
            .find(|instance| instance.id == instance_id)
    }

    pub fn get_instances(&self) -> &Vec<Instance> {
        &self.instances
    }

    pub fn log_event(
        &self,
        cx: &Context<Self>,
        event_type: &'static str,
        action_id: Option<String>,
        instance_id: Option<String>,
    ) {
        if let Some(conn) = self.conn() {
            cx.spawn(async move |this, cx| {
                let result = tokio::task::spawn_blocking(move || {
                    let connection = conn.lock().unwrap();
                    database::insert_event(
                        &connection,
                        event_type,
                        action_id.as_deref(),
                        instance_id.as_deref(),
                    )
                })
                .await;

                match result {
                    Ok(Err(error)) => {
                        let error_msg = format!("{error}");
                        if let Err(update_error) =
                            this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                        {
                            eprintln!("Failed to emit log event error: {update_error}");
                        }
                    }
                    Err(error) => {
                        let error_msg = format!("{error}");
                        if let Err(update_error) =
                            this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                        {
                            eprintln!("Failed to emit log event error: {update_error}");
                        }
                    }
                    Ok(Ok(())) => {}
                }
            })
            .detach();
        }
    }

    pub fn load_pipeline(&self, cx: &Context<Self>) {
        if let Some(conn) = self.conn() {
            cx.spawn(async move |this, cx| {
                let result = tokio::task::spawn_blocking(move || {
                    let connection = conn.lock().unwrap();
                    database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)
                })
                .await;

                match result {
                    Ok(Ok(items)) => {
                        if let Err(error) = this.update(cx, move |this, cx| {
                            this.pipeline_items = items;
                            cx.emit(PipelineLoaded);
                            cx.notify();
                        }) {
                            eprintln!("Failed to update pipeline: {error}");
                        }
                    }
                    Ok(Err(error)) => {
                        let error_msg = format!("{error}");
                        if let Err(update_error) =
                            this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                        {
                            eprintln!("Failed to emit pipeline load error: {update_error}");
                        }
                    }
                    Err(error) => {
                        let error_msg = format!("{error}");
                        if let Err(update_error) =
                            this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                        {
                            eprintln!("Failed to emit pipeline load error: {update_error}");
                        }
                    }
                }
            })
            .detach();
        }
    }

    pub fn get_pipeline_items(&self) -> &Vec<PipelineItem> {
        &self.pipeline_items
    }

    pub fn create_instance_for_action(&self, action_id: String, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        let Some(action) = self
            .actions
            .iter()
            .find(|action| action.id == action_id)
            .cloned()
        else {
            eprintln!("Unknown action id '{action_id}'");
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                let (instance, _pipeline_item) =
                    database::create_instance_and_enqueue(&connection, &action, "pending")?;

                let instances = database::fetch_instances(&connection)?;
                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>((instances, items))
            })
            .await;

            match result {
                Ok(Ok((instances, items))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.instances = instances;
                        this.pipeline_items = items;
                        cx.emit(ActionsLoaded);
                        cx.emit(PipelineLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update store after creation: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit instance creation error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit instance creation error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn enqueue_instance(&self, instance_id: String, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::enqueue_instance(&connection, &instance_id, None)?;

                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>(items)
            })
            .await;

            match result {
                Ok(Ok(items)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.pipeline_items = items;
                        cx.emit(PipelineLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update pipeline after enqueue: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit enqueue error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit enqueue error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn complete_pipeline_item(&self, instance_id: String, cx: &Context<Self>) {
        self.update_instance_status(instance_id, "completed", cx);
    }

    pub fn uncomplete_pipeline_item(&self, instance_id: String, cx: &Context<Self>) {
        self.update_instance_status(instance_id, "pending", cx);
    }

    fn update_instance_status(&self, instance_id: String, status: &str, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        let status_string = status.to_owned();

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::set_instance_status(&connection, &instance_id, &status_string)?;

                let instances = database::fetch_instances(&connection)?;
                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>((instances, items))
            })
            .await;

            match result {
                Ok(Ok((instances, items))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.instances = instances;
                        this.pipeline_items = items;
                        cx.emit(ActionsLoaded);
                        cx.emit(PipelineLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update store after status change: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit status update error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit status update error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn delete_pipeline_item(&self, pipeline_item_id: String, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::delete_pipeline_item(&connection, &pipeline_item_id)?;

                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>(items)
            })
            .await;

            match result {
                Ok(Ok(items)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.pipeline_items = items;
                        cx.emit(PipelineLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update pipeline after deletion: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit delete pipeline item error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit delete pipeline item error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn delete_instance(&self, instance_id: String, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::delete_instance(&connection, &instance_id)?;

                let instances = database::fetch_instances(&connection)?;
                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>((instances, items))
            })
            .await;

            match result {
                Ok(Ok((instances, items))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.instances = instances;
                        this.pipeline_items = items;
                        cx.emit(ActionsLoaded);
                        cx.emit(PipelineLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update store after deletion: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit delete error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit delete error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn delete_action(&self, action_id: String, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        let action_id_clone = action_id.clone();

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::delete_action(&connection, &action_id)
            })
            .await;

            match result {
                Ok(Ok(())) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.actions.retain(|action| action.id != action_id_clone);
                        this.load_all_actions(cx);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update store after action deletion: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit delete action error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit delete action error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn insert_instance_at_position(
        &self,
        action_id: String,
        position: i64,
        cx: &Context<Self>,
    ) {
        let Some(conn) = self.conn() else {
            return;
        };

        let Some(action) = self
            .actions
            .iter()
            .find(|action| action.id == action_id)
            .cloned()
        else {
            eprintln!("Unknown action id '{action_id}'");
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                let (instance, _pipeline_item) = database::insert_instance_at_position(
                    &connection,
                    &action,
                    "pending",
                    DEFAULT_PIPELINE_ID,
                    position,
                )?;

                let instances = database::fetch_instances(&connection)?;
                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>((instances, items))
            })
            .await;

            match result {
                Ok(Ok((instances, items))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.instances = instances;
                        this.pipeline_items = items;
                        cx.emit(ActionsLoaded);
                        cx.emit(PipelineLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update store after insertion: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit insert error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit insert error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn reorder_pipeline_item(&self, item_id: String, new_position: i64, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::update_pipeline_item_position(&connection, &item_id, new_position)?;

                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>(items)
            })
            .await;

            match result {
                Ok(Ok(items)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.pipeline_items = items;
                        cx.emit(PipelineLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update pipeline after reorder: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit reorder error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit reorder error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn normalize_positions(&self, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::normalize_pipeline_positions(&connection, DEFAULT_PIPELINE_ID)?;

                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>(items)
            })
            .await;

            match result {
                Ok(Ok(items)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.pipeline_items = items;
                        cx.emit(PipelineLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update pipeline after normalization: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit normalize error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit normalize error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    // ========================================================================
    // Phase 2: Context & Mental State
    // ========================================================================

    pub fn get_current_context(&self) -> Option<&ContextSnapshot> {
        self.current_context.as_ref()
    }

    pub fn get_context_energy(&self) -> Option<f64> {
        let snapshot = self.current_context.as_ref()?;
        let metadata_str = snapshot.metadata.as_ref()?;
        let metadata: serde_json::Value = serde_json::from_str(metadata_str).ok()?;
        metadata.get("energy").and_then(|v| v.as_f64())
    }

    pub fn get_context_attention(&self) -> Option<f64> {
        let snapshot = self.current_context.as_ref()?;
        let metadata_str = snapshot.metadata.as_ref()?;
        let metadata: serde_json::Value = serde_json::from_str(metadata_str).ok()?;
        metadata.get("attention").and_then(|v| v.as_f64())
    }

    pub fn get_mental_states(&self) -> &Vec<MentalState> {
        &self.mental_states
    }

    pub fn get_current_mental_state(&self) -> Option<&MentalState> {
        self.current_mental_state.as_ref()
    }

    pub fn get_pipeline_scores(&self) -> &Vec<(String, f64)> {
        &self.pipeline_scores
    }

    pub fn get_score_for_pipeline_item(&self, pipeline_item_id: &str) -> Option<f64> {
        self.pipeline_scores
            .iter()
            .find(|(id, _)| id == pipeline_item_id)
            .map(|(_, score)| *score)
    }

    pub fn get_suggestions(&self) -> &Vec<(Instance, Action, f64)> {
        &self.suggestions
    }

    pub fn get_routines(&self) -> &Vec<Routine> {
        &self.routines
    }

    pub fn get_routine(&self, routine_id: &str) -> Option<&Routine> {
        self.routines.iter().find(|r| r.id == routine_id)
    }

    pub fn get_routine_steps(&self) -> &Vec<RoutineStep> {
        &self.routine_steps
    }

    pub fn load_current_context(&self, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::fetch_current_context(&connection)
            })
            .await;

            match result {
                Ok(Ok(context)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.current_context = context;
                        cx.emit(ContextLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update current context: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit context load error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit context load error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn save_context_snapshot(&self, snapshot: ContextSnapshot, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::insert_context_snapshot(&connection, &snapshot)?;
                database::fetch_current_context(&connection)
            })
            .await;

            match result {
                Ok(Ok(context)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.current_context = context;
                        cx.emit(ContextLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update context after save: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit context save error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit context save error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn update_energy(&self, energy: f64, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::update_context_energy(&connection, energy)?;
                database::fetch_current_context(&connection)
            })
            .await;

            match result {
                Ok(Ok(context)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.current_context = context;
                        cx.emit(ContextLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update context after energy change: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit energy update error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit energy update error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn update_attention(&self, attention: f64, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::update_context_attention(&connection, attention)?;
                database::fetch_current_context(&connection)
            })
            .await;

            match result {
                Ok(Ok(context)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.current_context = context;
                        cx.emit(ContextLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update context after attention change: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit attention update error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit attention update error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn load_mental_states(&self, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::fetch_mental_states(&connection)
            })
            .await;

            match result {
                Ok(Ok(states)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.mental_states = states;
                        cx.emit(MentalStatesLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update mental states: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit mental states load error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit mental states load error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn load_current_mental_state(&self, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::fetch_current_mental_state(&connection)
            })
            .await;

            match result {
                Ok(Ok(state)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.current_mental_state = state;
                        cx.emit(MentalStatesLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update current mental state: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit current mental state load error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit current mental state load error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    /// Records a mental state event. If no mental state with the given name exists,
    /// creates it first, then records the event and updates the active mental state
    /// in the context snapshot.
    pub fn record_mental_state(
        &self,
        state_name: String,
        intensity: Option<i64>,
        cx: &Context<Self>,
    ) {
        let Some(conn) = self.conn() else {
            return;
        };

        let mental_states = self.mental_states.clone();

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();

                let state_id =
                    if let Some(existing) = mental_states.iter().find(|s| s.name == state_name) {
                        existing.id.clone()
                    } else {
                        let new_state = MentalState::new(&state_name);
                        database::insert_mental_state(&connection, &new_state)?
                    };

                let mut event = MentalStateEvent::new(&state_id);
                event.intensity = intensity;
                event.recorded_at = Some(chrono::Utc::now().to_rfc3339());
                database::insert_mental_state_event(&connection, &event)?;

                // Update the context snapshot's active mental state
                let current_context = database::fetch_current_context(&connection)?;
                if let Some(snapshot) = &current_context {
                    connection.execute(
                        "UPDATE context_snapshots SET active_mental_state = ?1 WHERE id = ?2",
                        (&state_id, &snapshot.id),
                    )?;
                } else {
                    let mut new_snapshot = ContextSnapshot::new();
                    new_snapshot.active_mental_state = Some(state_id);
                    new_snapshot.recorded_at = Some(chrono::Utc::now().to_rfc3339());
                    database::insert_context_snapshot(&connection, &new_snapshot)?;
                }

                let updated_context = database::fetch_current_context(&connection)?;
                let current_mental_state = database::fetch_current_mental_state(&connection)?;
                let all_states = database::fetch_mental_states(&connection)?;

                Ok::<_, anyhow::Error>((updated_context, current_mental_state, all_states))
            })
            .await;

            match result {
                Ok(Ok((context, mental_state, all_states))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.current_context = context;
                        this.current_mental_state = mental_state;
                        this.mental_states = all_states;
                        cx.emit(ContextLoaded);
                        cx.emit(MentalStatesLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update store after mental state recording: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit mental state recording error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit mental state recording error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    // ========================================================================
    // Phase 3: Scoring & Smart Pipeline
    // ========================================================================

    pub fn score_pipeline(&self, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::score_pipeline_items(&connection, DEFAULT_PIPELINE_ID)
            })
            .await;

            match result {
                Ok(Ok(scored)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.pipeline_scores = scored
                            .into_iter()
                            .map(|(item, score)| (item.id, score))
                            .collect();
                        cx.emit(PipelineScored);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update pipeline scores: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit scoring error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit scoring error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    /// Scores all pipeline items and reorders them by score (highest first),
    /// persisting the new positions in the database.
    pub fn refresh_pipeline(&self, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                let mut scored = database::score_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                for (position, (item, _score)) in scored.iter().enumerate() {
                    database::update_pipeline_item_position(
                        &connection,
                        &item.id,
                        (position as i64) + 1,
                    )?;
                }

                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;
                let scores: Vec<(String, f64)> = scored
                    .into_iter()
                    .map(|(item, score)| (item.id, score))
                    .collect();

                Ok::<_, anyhow::Error>((items, scores))
            })
            .await;

            match result {
                Ok(Ok((items, scores))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.pipeline_items = items;
                        this.pipeline_scores = scores;
                        cx.emit(PipelineLoaded);
                        cx.emit(PipelineScored);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update pipeline after refresh: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit refresh error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit refresh error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn suggest_next(&self, count: usize, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::suggest_best_instances(&connection, count)
            })
            .await;

            match result {
                Ok(Ok(suggestions)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.suggestions = suggestions;
                        cx.emit(SuggestionsLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update suggestions: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit suggest error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit suggest error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn explain_pipeline_item(
        &self,
        instance_id: String,
    ) -> Option<std::sync::Arc<std::sync::Mutex<Option<ScoredInstance>>>> {
        let conn = self.conn()?;
        let result_holder = std::sync::Arc::new(std::sync::Mutex::new(None));
        let holder = result_holder.clone();

        std::thread::spawn(move || {
            let connection = conn.lock().unwrap();
            match database::score_instance_with_context(&connection, &instance_id) {
                Ok(scored) => {
                    if let Ok(mut holder) = holder.lock() {
                        *holder = Some(scored);
                    }
                }
                Err(error) => {
                    eprintln!("Failed to explain pipeline item: {error}");
                }
            }
        });

        Some(result_holder)
    }

    // ========================================================================
    // Phase 4: Event Tracking
    // ========================================================================

    /// Completes an instance with a tracked event, then removes it from the pipeline.
    pub fn complete_with_event(&self, instance_id: String, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        let action_id = self
            .instances
            .iter()
            .find(|i| i.id == instance_id)
            .map(|i| i.action_id.clone());

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();

                database::insert_tracked_event(
                    &connection,
                    EventType::Completed,
                    Some(&instance_id),
                    action_id.as_deref(),
                    None,
                )?;

                database::set_instance_status(&connection, &instance_id, "completed")?;

                let instances = database::fetch_instances(&connection)?;
                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>((instances, items))
            })
            .await;

            match result {
                Ok(Ok((instances, items))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.instances = instances;
                        this.pipeline_items = items;
                        cx.emit(ActionsLoaded);
                        cx.emit(PipelineLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update store after completion: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit completion error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit completion error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn skip_instance(&self, instance_id: String, cx: &Context<Self>) {
        self.track_and_update_instance(instance_id, EventType::Skipped, "skipped", cx);
    }

    pub fn snooze_instance(&self, instance_id: String, cx: &Context<Self>) {
        self.track_and_update_instance(instance_id, EventType::Snoozed, "pending", cx);
    }

    pub fn abandon_instance(&self, instance_id: String, cx: &Context<Self>) {
        self.track_and_update_instance(instance_id, EventType::Abandoned, "abandoned", cx);
    }

    fn track_and_update_instance(
        &self,
        instance_id: String,
        event_type: EventType,
        new_status: &str,
        cx: &Context<Self>,
    ) {
        let Some(conn) = self.conn() else {
            return;
        };

        let action_id = self
            .instances
            .iter()
            .find(|i| i.id == instance_id)
            .map(|i| i.action_id.clone());

        let status = new_status.to_owned();

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();

                database::insert_tracked_event(
                    &connection,
                    event_type,
                    Some(&instance_id),
                    action_id.as_deref(),
                    None,
                )?;

                database::set_instance_status(&connection, &instance_id, &status)?;

                // Remove from pipeline for skip/abandon (snoozed stays as pending)
                if status == "skipped" || status == "abandoned" {
                    let pipeline_items =
                        database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;
                    for item in &pipeline_items {
                        if item.instance_id.as_deref() == Some(&instance_id) {
                            database::delete_pipeline_item(&connection, &item.id)?;
                            break;
                        }
                    }
                }

                let instances = database::fetch_instances(&connection)?;
                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>((instances, items))
            })
            .await;

            match result {
                Ok(Ok((instances, items))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.instances = instances;
                        this.pipeline_items = items;
                        cx.emit(ActionsLoaded);
                        cx.emit(PipelineLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update store after event tracking: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit event tracking error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit event tracking error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn load_routines(&self, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::fetch_routines(&connection)
            })
            .await;

            match result {
                Ok(Ok(routines)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.routines = routines;
                        cx.emit(RoutinesLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update routines: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit routine load error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit routine load error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn load_routine_steps(&self, routine_id: String, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        let routine_id_for_event = routine_id.clone();

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::fetch_routine_steps(&connection, &routine_id)
            })
            .await;

            match result {
                Ok(Ok(steps)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.routine_steps = steps;
                        cx.emit(RoutineStepsLoaded {
                            routine_id: routine_id_for_event,
                        });
                        cx.notify();
                    }) {
                        eprintln!("Failed to update routine steps: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit routine steps load error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit routine steps load error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn create_routine(&self, routine: Routine, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::insert_routine(&connection, &routine)?;
                database::fetch_routines(&connection)
            })
            .await;

            match result {
                Ok(Ok(routines)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.routines = routines;
                        cx.emit(RoutinesLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update routines after create: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit routine create error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit routine create error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn delete_routine(&self, routine_id: String, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::delete_routine(&connection, &routine_id)?;
                database::fetch_routines(&connection)
            })
            .await;

            match result {
                Ok(Ok(routines)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.routines = routines;
                        cx.emit(RoutinesLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update routines after delete: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit routine delete error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit routine delete error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn add_routine_step(&self, routine_id: String, action_id: String, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        let routine_id_for_reload = routine_id.clone();

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                let step_order = database::next_routine_step_order(&connection, &routine_id)?;
                let step = RoutineStep::new(&routine_id, &action_id, step_order);
                database::insert_routine_step(&connection, &step)?;
                let steps = database::fetch_routine_steps(&connection, &routine_id)?;
                let routines = database::fetch_routines(&connection)?;
                Ok::<_, anyhow::Error>((steps, routines))
            })
            .await;

            match result {
                Ok(Ok((steps, routines))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.routine_steps = steps;
                        this.routines = routines;
                        cx.emit(RoutineStepsLoaded {
                            routine_id: routine_id_for_reload,
                        });
                        cx.emit(RoutinesLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update after adding routine step: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit add step error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit add step error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn remove_routine_step(&self, step_id: String, routine_id: String, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        let routine_id_for_reload = routine_id.clone();

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::delete_routine_step(&connection, &step_id)?;
                let steps = database::fetch_routine_steps(&connection, &routine_id)?;
                let routines = database::fetch_routines(&connection)?;
                Ok::<_, anyhow::Error>((steps, routines))
            })
            .await;

            match result {
                Ok(Ok((steps, routines))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.routine_steps = steps;
                        this.routines = routines;
                        cx.emit(RoutineStepsLoaded {
                            routine_id: routine_id_for_reload,
                        });
                        cx.emit(RoutinesLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update after removing routine step: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit remove step error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit remove step error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }

    pub fn start_routine(&self, routine_id: String, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                let instantiation_result = database::instantiate_routine_by_id(
                    &connection,
                    &routine_id,
                    InstantiateRoutineOptions::default(),
                )?;

                let instances = database::fetch_instances(&connection)?;
                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>((instantiation_result, instances, items))
            })
            .await;

            match result {
                Ok(Ok((_instantiation_result, instances, items))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.instances = instances;
                        this.pipeline_items = items;
                        cx.emit(ActionsLoaded);
                        cx.emit(PipelineLoaded);
                        cx.notify();
                    }) {
                        eprintln!("Failed to update store after starting routine: {error}");
                    }
                }
                Ok(Err(error)) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit start routine error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit start routine error: {update_error}");
                    }
                }
            }
        })
        .detach();
    }
}

impl EventEmitter<DatabaseError> for DatabaseStore {}
impl EventEmitter<ActionsLoaded> for DatabaseStore {}
impl EventEmitter<PipelineLoaded> for DatabaseStore {}
impl EventEmitter<ContextLoaded> for DatabaseStore {}
impl EventEmitter<MentalStatesLoaded> for DatabaseStore {}
impl EventEmitter<PipelineScored> for DatabaseStore {}
impl EventEmitter<SuggestionsLoaded> for DatabaseStore {}
impl EventEmitter<RoutinesLoaded> for DatabaseStore {}
impl EventEmitter<RoutineStepsLoaded> for DatabaseStore {}
