use anyhow::Result;
// use chrono::Utc;
use gpui::{Context, EventEmitter};
use std::collections::HashMap;
use ticks::tasks::TaskID;
use ticks::{AccessToken, TickTick};

use crate::app::ResultExt;
use crate::auth;
// use crate::stores::ui_store::ViewType;
use crate::tasks::{TaskData, fetch_all_tasks, task_data_compare};

pub struct TasksUpdated;
// pub struct TaskCreated;
// pub struct TaskDeleted;
// pub struct TaskMoved(TaskID);
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

#[derive(Debug, Clone)]
struct ActiveDrag {
    // pub source: TaskLocation,
    pub task: TaskID,
    pub drop_target: Option<TaskLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskLocation {
    TaskList,
    Pipeline(usize),
}

pub struct TaskStore {
    api_client: Option<TickTick>,
    access_token: Option<AccessToken>,
    tasks: HashMap<TaskID, TaskData>,
    task_list: im::Vector<TaskID>,
    pipeline: im::Vector<TaskID>,
    active_drag: Option<ActiveDrag>,
}

impl TaskStore {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut store = Self {
            api_client: None,
            access_token: None,
            tasks: HashMap::new(),
            task_list: im::Vector::new(),
            pipeline: im::Vector::new(),
            active_drag: None,
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
                            this.refresh_tasks(cx).log_err();
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

    pub fn refresh_tasks(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let client = match &self.api_client {
            Some(client) => client.clone(),
            None => {
                cx.emit(ApiError {
                    message: "API client not initialized".to_string(),
                });
                cx.notify();
                return Ok(());
            }
        };

        let spawned = cx.spawn(async move |this, cx| match fetch_all_tasks(&client).await {
            Ok(tasks) => {
                this.update(cx, |this, cx| {
                    this.tasks.clear();
                    // for task in tasks {
                    //     let task_data = TaskData::from_task(&task);
                    //     let task_location = TaskLocation::TaskList;
                    //     if let Some(task_id) = &task_data.task_id {
                    //         this.tasks
                    //             .insert(task_id.clone(), (task_data, task_location));
                    //     }
                    // }
                    for (i, task) in tasks.iter().enumerate() {
                        let task_data = TaskData::from_task(&task);
                        // let task_location = TaskLocation::TaskList;
                        let task_location = TaskLocation::Pipeline(i);
                        if let Some(task_id) = &task_data.task_id {
                            this.tasks.insert(task_id.clone(), task_data);
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
        Ok(())
    }

    pub fn task_list_data(&self) -> Vec<TaskData> {
        let mut data = self
            .tasks
            .iter()
            // .filter(|(_, (_, location))| matches!(location, Some(TaskLocation::TaskList)))
            .map(|(_, task_data)| task_data)
            .cloned()
            .collect::<Vec<TaskData>>();

        data.sort_by(task_data_compare);
        data
    }

    pub fn pipeline_data(&self) -> Vec<TaskData> {
        let mut data = self
            .tasks
            .iter()
            // .filter(|(_, (_, location))| matches!(location, Some(TaskLocation::Pipeline(_))))
            .map(|(_, task_data)| task_data)
            .cloned()
            .collect::<Vec<TaskData>>();

        // // Sort by pipeline index
        // data.sort_by_key(|(_, location)| {
        //     if let Some(TaskLocation::Pipeline(index)) = location {
        //         *index
        //     } else {
        //         usize::MAX
        //     }
        // });

        // data.into_iter().map(|(task_data, _)| task_data).collect()
        data.sort_by(task_data_compare);
        data
    }

    pub fn update_location(
        &mut self,
        task_id: &TaskID,
        new_location: Option<TaskLocation>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        Ok(())
    }

    // pub fn get_tasks(&self) -> Vec<&TaskData> {
    //     self.tasks.values().collect()
    // }

    // /// Filter tasks based on the selected view type
    // /// - TaskList: All tasks
    // /// - Today: Overdue tasks and tasks due today
    // /// - Upcoming: Tasks due in the next 7 days (excluding today's and overdue)
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

    // pub fn get_all_tasks(&self) -> Vec<&TaskData> {
    //     self.tasks.values().collect()
    // }

    // pub fn get_task_by_id(&self, task_id: &str) -> Option<&TaskData> {
    //     self.tasks.get(task_id)
    // }
}

impl EventEmitter<TasksUpdated> for TaskStore {}
// impl EventEmitter<TaskCreated> for TaskStore {}
// impl EventEmitter<TaskDeleted> for TaskStore {}
impl EventEmitter<ApiError> for TaskStore {}
