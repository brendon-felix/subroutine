use anyhow::Result;
use gpui::{Context, EventEmitter};
use ticks::{AccessToken, TickTick};

use crate::auth;

#[derive(Clone, Debug)]
pub struct TasksUpdated;

#[derive(Clone, Debug)]
pub struct TaskCreated;

#[derive(Clone, Debug)]
pub struct TaskDeleted;

#[derive(Clone, Debug)]
pub struct ApiError {
    pub message: String,
}

// #[derive(Clone, Debug)]
// pub struct CreateTaskRequest {
//     pub title: String,
//     pub content: Option<String>,
//     pub due_date: Option<DateTime<Utc>>,
//     pub priority: Option<u8>,
// }

trait ResultExt<T> {
    fn log_err(self);
}

impl<T> ResultExt<T> for Result<T> {
    fn log_err(self) {
        if let Err(err) = self {
            eprintln!("Error: {}", err);
        }
    }
}

pub struct TaskStore {
    api_client: Option<TickTick>,
    access_token: Option<AccessToken>,
}

impl TaskStore {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut store = Self {
            api_client: None,
            access_token: None,
            // error: None,
        };

        store.initialize_client(cx);

        store
    }

    fn initialize_client(&mut self, cx: &mut Context<Self>) {
        let (client_id, client_secret) = match auth::get_client_id() {
            Ok((id, secret)) => (id, secret),
            Err(e) => {
                cx.emit(ApiError {
                    message: format!("Authentication credentials not found: {}", e),
                });
                cx.notify();
                return;
            }
        };

        let spawned = cx.spawn(async move |this, cx| {
            match auth::get_access_token(client_id, client_secret).await {
                Ok(access_token) => {
                    this.update(cx, |this, cx| match TickTick::new(access_token.clone()) {
                        Ok(client) => {
                            this.api_client = Some(client);
                            this.access_token = Some(access_token);
                            cx.notify();
                            this.refresh_tasks(cx);
                        }
                        Err(e) => {
                            cx.emit(ApiError {
                                message: format!("Failed to create TickTick client: {}", e),
                            });
                            cx.notify();
                        }
                    })
                    .log_err();
                }
                Err(e) => {
                    this.update(cx, |_this, cx| {
                        cx.emit(ApiError {
                            message: format!("Failed to get access token: {}", e),
                        });
                        cx.notify();
                    })
                    .log_err();
                }
            }
        });

        spawned.detach();
    }

    pub fn refresh_tasks(&mut self, _cx: &mut Context<Self>) {
        // task fetching logic here
    }
}

impl EventEmitter<TasksUpdated> for TaskStore {}
impl EventEmitter<TaskCreated> for TaskStore {}
impl EventEmitter<TaskDeleted> for TaskStore {}
impl EventEmitter<ApiError> for TaskStore {}
