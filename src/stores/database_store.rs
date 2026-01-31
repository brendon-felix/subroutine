use gpui::{Context, EventEmitter};

use database::{
    self, Action, DEFAULT_PIPELINE_ID, DatabaseConnection, Instance, PipelineItem, Routine,
};

pub struct DatabaseError {
    pub message: String,
}

pub struct ActionsLoaded;

pub struct PipelineLoaded;

pub struct DatabaseStore {
    conn: Option<DatabaseConnection>,
    actions: Vec<Action>,
    instances: Vec<Instance>,
    pipeline_items: Vec<PipelineItem>,
}

impl DatabaseStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            conn: None,
            actions: Vec::new(),
            instances: Vec::new(),
            pipeline_items: Vec::new(),
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
                        cx.notify();
                    }) {
                        eprintln!("Failed to store database connection: {error}");
                    }
                    println!("Database initialized successfully\n");
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
            println!("Loading actions...");

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
                            println!("Loaded {} actions\n", this.actions.len());
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

    pub fn get_actions(&self) -> &Vec<Action> {
        &self.actions
    }

    pub fn load_all_instances(&self, cx: &Context<Self>) {
        if let Some(conn) = self.conn() {
            println!("Loading instances...");
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
                            println!("Loaded {} instances\n", this.instances.len());
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
            println!("Loading pipeline...");
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
                            println!("Loaded {} pipeline items", this.pipeline_items.len());
                            cx.emit(PipelineLoaded);
                            cx.notify();
                        }) {
                            eprintln!("Failed to update pipeline items: {error}");
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

    pub fn create_instance_for_action(&self, action_id: String, cx: &mut Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        let Some(action) = self
            .actions
            .iter()
            .find(|action| action.id == action_id)
            .cloned()
        else {
            cx.emit(DatabaseError {
                message: format!("Unknown action id '{action_id}'"),
            });
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                let (instance, _pipeline_item) =
                    database::create_instance_and_enqueue(&connection, &action, "pending")?;

                println!("Created new instance {}", &instance.id);

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
                        println!("Reloaded instances and pipeline after creation");
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
                        eprintln!("Failed to emit creation error: {update_error}");
                    }
                }
                Err(error) => {
                    let error_msg = format!("{error}");
                    if let Err(update_error) =
                        this.update(cx, |_, cx| Self::emit_error(cx, error_msg))
                    {
                        eprintln!("Failed to emit creation error: {update_error}");
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

        let action_title = self
            .instances
            .iter()
            .find(|instance| instance.id == instance_id)
            .and_then(|instance| {
                self.actions
                    .iter()
                    .find(|action| action.id == instance.action_id)
                    .map(|action| action.title.clone())
            });

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::enqueue_instance(&connection, &instance_id, action_title.as_deref())?;

                println!("Enqueued instance {instance_id}\n");

                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>(items)
            })
            .await;

            match result {
                Ok(Ok(items)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.pipeline_items = items;
                        println!("Reloaded pipeline after enqueue");
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

                println!(
                    "Updated instance {} status to {}",
                    instance_id, status_string
                );

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
                        println!("Reloaded instances and pipeline after status change");
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

                println!("Deleted pipeline item {}", pipeline_item_id);

                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>(items)
            })
            .await;

            match result {
                Ok(Ok(items)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.pipeline_items = items;
                        println!("Reloaded pipeline after item deletion");
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
        println!("[DB_STORE] delete_instance called for {}", instance_id);

        let Some(conn) = self.conn() else {
            return;
        };

        let instance_id_clone = instance_id.clone();
        let instance_id_clone2 = instance_id_clone.clone();
        let instance_id_clone3 = instance_id_clone.clone();

        cx.spawn(async move |this, cx| {
            println!(
                "[DB_STORE] delete_instance spawn started for {}",
                instance_id_clone
            );

            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::delete_instance(&connection, &instance_id)?;
                println!("Deleted instance {}", instance_id);

                let instances = database::fetch_instances(&connection)?;
                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>((instances, items))
            })
            .await;

            println!(
                "[DB_STORE] delete_instance attempting entity update for {}",
                instance_id_clone2
            );
            match result {
                Ok(Ok((instances, items))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        println!("[DB_STORE] Inside entity update closure after delete");
                        this.instances = instances;
                        this.pipeline_items = items;
                        println!("Reloaded instances and pipeline after deletion");
                        cx.emit(ActionsLoaded);
                        cx.emit(PipelineLoaded);
                        cx.notify();
                        println!("[DB_STORE] Emitted PipelineLoaded after delete");
                    }) {
                        eprintln!("[DB_STORE] Failed to update store after deletion: {error}");
                    } else {
                        println!(
                            "[DB_STORE] delete_instance entity update succeeded for {}",
                            instance_id_clone2
                        );
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

            println!(
                "[DB_STORE] delete_instance spawn completed for {}",
                instance_id_clone2
            );
        })
        .detach();
        println!(
            "[DB_STORE] delete_instance method exiting (spawn detached) for {}",
            instance_id_clone3
        );
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
                        println!("Deleted action {}", action_id_clone);
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
        println!(
            "[DB_STORE] insert_instance_at_position called for action {} at position {}",
            action_id, position
        );

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

        let action_id_clone = action_id.clone();
        let action_id_clone2 = action_id_clone.clone();

        cx.spawn(async move |this, cx| {
            println!(
                "[DB_STORE] insert_instance_at_position spawn started for action {}",
                action_id_clone
            );

            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                let (instance, _pipeline_item) = database::insert_instance_at_position(
                    &connection,
                    &action,
                    "pending",
                    DEFAULT_PIPELINE_ID,
                    position,
                )?;

                println!(
                    "Created new instance {} at position {}",
                    &instance.id, position
                );

                let instances = database::fetch_instances(&connection)?;
                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>((instances, items))
            })
            .await;

            println!("[DB_STORE] insert_instance_at_position attempting entity update");
            match result {
                Ok(Ok((instances, items))) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        println!("[DB_STORE] Inside entity update closure after insert");
                        this.instances = instances;
                        this.pipeline_items = items;
                        println!("Reloaded instances and pipeline after insertion");
                        cx.emit(ActionsLoaded);
                        cx.emit(PipelineLoaded);
                        cx.notify();
                        println!("[DB_STORE] Emitted PipelineLoaded after insert");
                    }) {
                        eprintln!("[DB_STORE] Failed to update store after insertion: {error}");
                    } else {
                        println!("[DB_STORE] insert_instance_at_position entity update succeeded");
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

            println!(
                "[DB_STORE] insert_instance_at_position spawn completed for action {}",
                action_id_clone
            );
        })
        .detach();
        println!(
            "[DB_STORE] insert_instance_at_position method exiting (spawn detached) for action {}",
            action_id_clone2
        );
    }

    pub fn reorder_pipeline_item(&self, item_id: String, new_position: i64, cx: &Context<Self>) {
        let Some(conn) = self.conn() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let connection = conn.lock().unwrap();
                database::update_pipeline_item_position(&connection, &item_id, new_position)?;

                println!(
                    "Reordered pipeline item {} to position {}",
                    item_id, new_position
                );

                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>(items)
            })
            .await;

            match result {
                Ok(Ok(items)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.pipeline_items = items;
                        println!("Reloaded pipeline after reorder");
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

                println!("Normalized pipeline positions");

                let items = database::fetch_pipeline_items(&connection, DEFAULT_PIPELINE_ID)?;

                Ok::<_, anyhow::Error>(items)
            })
            .await;

            match result {
                Ok(Ok(items)) => {
                    if let Err(error) = this.update(cx, move |this, cx| {
                        this.pipeline_items = items;
                        println!("Reloaded pipeline after normalization");
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
}

impl EventEmitter<DatabaseError> for DatabaseStore {}
impl EventEmitter<ActionsLoaded> for DatabaseStore {}
impl EventEmitter<PipelineLoaded> for DatabaseStore {}
