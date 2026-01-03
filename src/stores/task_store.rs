use anyhow::Result;
// use chrono::Utc;
use gpui::{Context, EventEmitter};
use std::collections::HashMap;
use ticks::{AccessToken, TickTick};

use crate::auth;
// use crate::stores::ui_store::ViewType;
use crate::tasks::{TaskData, fetch_all_tasks};

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
    tasks: HashMap<String, TaskData>,
}

impl TaskStore {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut store = Self {
            api_client: None,
            access_token: None,
            tasks: HashMap::new(),
        };

        store.initialize_client(cx);

        store
    }

    // pub fn is_loading(&self) -> bool {
    //     self.api_client.is_none()
    // }

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

    pub fn refresh_tasks(&mut self, cx: &mut Context<Self>) {
        if let Some(ref client) = self.api_client {
            let client = client.clone();
            let spawned = cx.spawn(async move |this, cx| match fetch_all_tasks(&client).await {
                Ok(tasks) => {
                    this.update(cx, |this, cx| {
                        this.tasks.clear();
                        for task in tasks {
                            let task_data = TaskData::from_task(&task);
                            if let Some(task_id) = &task_data.task_id {
                                this.tasks.insert(task_id.0.clone(), task_data);
                            }
                        }
                        cx.emit(TasksUpdated);
                        cx.notify();
                    })
                    .log_err();
                }
                Err(e) => {
                    this.update(cx, |_this, cx| {
                        cx.emit(ApiError {
                            message: format!("Failed to fetch tasks: {}", e),
                        });
                        cx.notify();
                    })
                    .log_err();
                }
            });
            spawned.detach();
        }
    }

    // pub fn get_tasks(&self) -> Vec<&TaskData> {
    //     self.tasks.values().collect()
    // }

    /// Filter tasks based on the selected view type
    /// - TaskList: All tasks
    /// - Today: Overdue tasks and tasks due today
    /// - Upcoming: Tasks due in the next 7 days (excluding today's and overdue)
    // pub fn get_filtered_tasks(&self, view_type: &ViewType) -> Vec<&TaskData> {
    //     let now = Utc::now();
    //     let today_end = now.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();
    //     let next_week = now + chrono::Duration::days(7);

    //     match view_type {
    //         // Show overdue tasks and tasks due today
    //         ViewType::Today => {
    //             self.tasks
    //                 .values()
    //                 .filter(|task| {
    //                     if let Some(due_date) = task.due_date {
    //                         // Include tasks due today or earlier (overdue)
    //                         due_date <= today_end
    //                     } else {
    //                         // Exclude tasks without due dates
    //                         false
    //                     }
    //                 })
    //                 .collect()
    //         }
    //         // Show tasks due in the next week (excluding today's and overdue)
    //         ViewType::Upcoming => {
    //             self.tasks
    //                 .values()
    //                 .filter(|task| {
    //                     if let Some(due_date) = task.due_date {
    //                         // Include tasks due after today but within the next week
    //                         due_date > today_end && due_date <= next_week
    //                     } else {
    //                         // Exclude tasks without due dates
    //                         false
    //                     }
    //                 })
    //                 .collect()
    //         }
    //         // Show tasks in the Inbox
    //         ViewType::Inbox => self
    //             .tasks
    //             .values()
    //             .filter(|task| task.project_id.as_ref().unwrap().0.starts_with("inbox"))
    //             .collect(),
    //         // Show all tasks without filtering
    //         ViewType::AllTasks => self.tasks.values().collect(),
    //     }
    // }

    pub fn get_all_tasks(&self) -> Vec<&TaskData> {
        self.tasks.values().collect()
    }

    // pub fn get_task_by_id(&self, task_id: &str) -> Option<&TaskData> {
    //     self.tasks.get(task_id)
    // }
}

impl EventEmitter<TasksUpdated> for TaskStore {}
impl EventEmitter<TaskCreated> for TaskStore {}
impl EventEmitter<TaskDeleted> for TaskStore {}
impl EventEmitter<ApiError> for TaskStore {}
