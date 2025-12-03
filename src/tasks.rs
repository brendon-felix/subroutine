use chrono::{DateTime, Utc};
use ticks::{
    projects::ProjectID,
    tasks::{Task, TaskID, TaskPriority},
};

#[derive(Debug, Clone, Default)]
pub struct TaskData {
    pub title: Option<String>,
    pub task_id: Option<TaskID>,
    pub project_id: Option<ProjectID>, // None means inbox
    // is_all_day: bool, // assumed true if time is 00:00:00
    // completed_time: Option<DateTime<Utc>>, // APi doesn't support completed tasks afaik
    pub content: Option<String>,
    // desc: Option<String>, // not really sure what this field does
    pub due_date: Option<DateTime<Utc>>,
    // subtasks: Vec<Subtask>, // not supported yet
    pub priority: Option<TaskPriority>,
    pub repeat_flag: Option<String>,
    // reminders: Option<Vec<String>>, // not supported yet
    // sort_order, Option<i64>, // not supported
    // start_date: Option<DateTime<Utc>>, // not supported yet
    // status: Option<TaskStatus>, // again, not sure API even sends completed tasks
    // time_zone: Option<chrono::TimeZone>, // assume local timezone
    // tags: Vec<String>, // also don't think API supports this
}

#[allow(dead_code)]
impl TaskData {
    pub fn from_task(task: &Task) -> Self {
        Self {
            title: Some(task.title.clone()),
            task_id: Some(task.get_id().clone()),
            project_id: Some(task.project_id.clone()),
            content: Some(task.content.clone()),
            due_date: Some(task.due_date),
            priority: Some(task.priority),
            repeat_flag: if task.repeat_flag.is_empty() {
                None
            } else {
                Some(task.repeat_flag.clone())
            },
        }
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn task_id(mut self, task_id: TaskID) -> Self {
        self.task_id = Some(task_id);
        self
    }

    pub fn project_id(mut self, project_id: ProjectID) -> Self {
        self.project_id = Some(project_id);
        self
    }

    pub fn content(mut self, content: String) -> Self {
        self.content = Some(content);
        self
    }

    pub fn due_date(mut self, due_date: DateTime<Utc>) -> Self {
        self.due_date = Some(due_date);
        self
    }

    pub fn priority(mut self, priority: TaskPriority) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn repeat_flag(mut self, repeat_flag: String) -> Self {
        self.repeat_flag = Some(repeat_flag);
        self
    }
}

// pub fn patch_task(task: &Task, data: TaskData) -> TaskData {
//     let mut new_data = TaskData::from_task(task);

//     if let Some(ref title) = data.title {
//         new_data.title = Some(title.clone());
//     }
//     if let Some(ref content) = data.content {
//         new_data.content = Some(content.clone());
//     }
//     if let Some(ref due_date) = data.due_date {
//         new_data.due_date = Some(*due_date);
//     }
//     if let Some(ref priority) = data.priority {
//         new_data.priority = Some(*priority);
//     }

//     new_data
// }

// #[derive(Debug, Clone)]
// pub enum TaskAction {
//     Create,
//     Edit,
//     Complete,
//     Delete,
//     // Reschedule(RescheduleTarget),
// }

// pub async fn fetch_all_tasks(client: &TickTick) -> Result<Vec<Task>> {
//     let project_tasks = client
//         .get_all_tasks_in_projects()
//         .await
//         .map_err(|e| anyhow::anyhow!("Failed to fetch tasks from projects: {:?}", e))?;
//     let inbox_id = ProjectID("inbox".to_string());
//     let inbox_tasks = client
//         .get_project_data(&inbox_id)
//         .await
//         .map_err(|e| anyhow::anyhow!("Failed to fetch inbox tasks: {:?}", e))?
//         .tasks;

//     let mut all_tasks: Vec<Task> = project_tasks
//         .into_iter()
//         .chain(inbox_tasks.into_iter())
//         .collect();

//     sort_tasks(&mut all_tasks);

//     Ok(all_tasks)
// }

// pub fn get_local_date(dt: DateTime<Utc>) -> NaiveDate {
//     dt.with_timezone(&Local).date_naive()
// }

// pub fn with_local_hms(date: NaiveDate, hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
//     let local_dt = date
//         .and_hms_opt(hour, min, sec)
//         .unwrap()
//         .and_local_timezone(Local)
//         .unwrap();
//     local_dt.with_timezone(&Utc)
// }

// pub fn is_overdue(now: DateTime<Utc>, task: &Task) -> bool {
//     let today_start = with_local_hms(get_local_date(now), 0, 0, 0);
//     if task.due_date.timestamp() > 0 && task.due_date < now {
//         task.due_date != today_start
//     } else {
//         false
//     }
// }

// pub fn is_due_today(now: DateTime<Utc>, task: &Task) -> bool {
//     let today_start = with_local_hms(get_local_date(now), 0, 0, 0);
//     let today_end = with_local_hms(get_local_date(now), 23, 59, 59);
//     task.due_date >= today_start && task.due_date <= today_end
// }

// pub fn is_due_tomorrow(now: DateTime<Utc>, task: &Task) -> bool {
//     let tomorrow = now + Duration::days(1);
//     let tomorrow_start = with_local_hms(get_local_date(tomorrow), 0, 0, 0);
//     let tomorrow_end = with_local_hms(get_local_date(tomorrow), 23, 59, 59);

//     task.due_date.timestamp() > 0
//         && task.due_date >= tomorrow_start
//         && task.due_date <= tomorrow_end
// }

// pub fn is_due_this_week(now: DateTime<Utc>, task: &Task) -> bool {
//     let today_end = with_local_hms(get_local_date(now), 23, 59, 59);
//     let next_week = now + Duration::days(7);
//     let week_end = with_local_hms(get_local_date(next_week), 23, 59, 59);

//     task.due_date.timestamp() > 0 && task.due_date >= today_end && task.due_date <= week_end
// }

// pub fn is_in_inbox(task: &Task) -> bool {
//     task.project_id.0.starts_with("inbox")
// }

// pub fn is_in_project(task: &Task, project_id: &ProjectID) -> bool {
//     &task.project_id.0 == &project_id.0
// }

// pub async fn create_task(client: &TickTick, data: TaskData) -> Result<(), String> {
//     let mut builder = ticks::tasks::Task::builder(client, data.title.as_ref().unwrap());
//     let project_id = data
//         .project_id
//         .clone()
//         .unwrap_or(ProjectID("inbox".to_string()));
//     builder = builder.project_id(project_id);

//     if let Some(c) = data.content {
//         builder = builder.content(&c);
//     }

//     if let Some(due_date) = data.due_date {
//         builder = builder.due_date(due_date);
//         builder = builder.start_date(due_date);
//         // if time is 12:00 AM, set as all-day
//         let time = due_date.with_timezone(&chrono::Local).time();
//         if time == NaiveTime::from_hms_opt(0, 0, 0).unwrap() {
//             builder = builder.is_all_day(true);
//         }
//     }

//     if let Some(priority) = data.priority {
//         builder = builder.priority(priority);
//     }

//     if let Some(repeat_flag) = data.repeat_flag {
//         builder = builder.repeat_flag(&repeat_flag);
//     }

//     match builder.build_and_publish().await {
//         Ok(_) => Ok(()),
//         Err(e) => Err(format!("Failed to create task: {:?}", e)),
//     }
// }

// pub async fn edit_task(client: &TickTick, data: TaskData) -> Result<(), String> {
//     // Get a fresh task instance from the API with proper client context
//     let project_id = data.project_id.clone().unwrap();
//     let task_id = data.task_id.clone().unwrap();
//     match client.get_project_data(&project_id).await {
//         Ok(project_data) => {
//             // Find the task in the project data
//             if let Some(mut task) = project_data.tasks.into_iter().find(|t| {
//                 let t_id = t.get_id();
//                 t_id == &task_id
//             }) {
//                 // Patch the task with new data
//                 let patched_data = patch_task(&task, data);

//                 if let Some(title) = patched_data.title {
//                     task.title = title;
//                 }
//                 if let Some(content) = patched_data.content {
//                     task.content = content;
//                 }
//                 if let Some(due_date) = patched_data.due_date {
//                     if task.start_date == task.due_date {
//                         task.start_date = due_date;
//                     }
//                     task.due_date = due_date;
//                 }
//                 if let Some(priority) = patched_data.priority {
//                     task.priority = priority;
//                 }

//                 match task.publish_changes().await {
//                     Ok(_) => Ok(()),
//                     Err(e) => Err(format!("Failed to edit task: {:?}", e)),
//                 }
//             } else {
//                 Err("Task not found in project".to_string())
//             }
//         }
//         Err(e) => Err(format!("Failed to get project data: {:?}", e)),
//     }
// }

// pub async fn complete_task(client: &TickTick, data: TaskData) -> Result<(), String> {
//     // Get a fresh task instance from the API with proper client context
//     let project_id = data.project_id.clone().unwrap();
//     let task_id = data.task_id.clone().unwrap();
//     match client.get_project_data(&project_id).await {
//         Ok(project_data) => {
//             // Find the task in the project data
//             if let Some(mut task) = project_data.tasks.into_iter().find(|t| {
//                 let t_id = t.get_id();
//                 t_id == &task_id
//             }) {
//                 match task.complete().await {
//                     Ok(_) => Ok(()),
//                     Err(e) => Err(format!("Failed to complete task: {:?}", e)),
//                 }
//             } else {
//                 Err("Task not found in project".to_string())
//             }
//         }
//         Err(e) => Err(format!("Failed to get project data: {:?}", e)),
//     }
// }

// pub async fn delete_task(client: &TickTick, data: TaskData) -> Result<(), String> {
//     // Get a fresh task instance from the API with proper client context
//     let project_id = data.project_id.clone().unwrap();
//     let task_id = data.task_id.clone().unwrap();
//     match client.get_project_data(&project_id).await {
//         Ok(project_data) => {
//             // Find the task in the project data
//             if let Some(task) = project_data.tasks.into_iter().find(|t| {
//                 let t_id = t.get_id();
//                 t_id == &task_id
//             }) {
//                 match task.delete().await {
//                     Ok(_) => Ok(()),
//                     Err(e) => Err(format!("Failed to delete task: {:?}", e)),
//                 }
//             } else {
//                 Err("Task not found in project".to_string())
//             }
//         }
//         Err(e) => Err(format!("Failed to get project data: {:?}", e)),
//     }
// }

// pub fn sort_tasks(tasks: &mut Vec<Task>) {
//     tasks.sort_by(|a, b| {
//         use chrono::{DateTime, Datelike, Utc};

//         // Helper to check if a datetime is the epoch (unset)
//         let is_unset = |dt: &DateTime<Utc>| dt.timestamp() == 0;

//         // Helper to compare dates by day only (year, month, day)
//         let compare_by_day = |dt_a: &DateTime<Utc>, dt_b: &DateTime<Utc>| {
//             match dt_a.year().cmp(&dt_b.year()) {
//                 std::cmp::Ordering::Equal => {}
//                 other => return other,
//             }
//             match dt_a.month().cmp(&dt_b.month()) {
//                 std::cmp::Ordering::Equal => {}
//                 other => return other,
//             }
//             dt_a.day().cmp(&dt_b.day())
//         };

//         // Compare due dates (unset dates go to the end)
//         let due_cmp = match (is_unset(&a.due_date), is_unset(&b.due_date)) {
//             (true, true) => std::cmp::Ordering::Equal,
//             (true, false) => std::cmp::Ordering::Greater,
//             (false, true) => std::cmp::Ordering::Less,
//             (false, false) => {
//                 // First compare by day
//                 let day_cmp = compare_by_day(&a.due_date, &b.due_date);
//                 if day_cmp != std::cmp::Ordering::Equal {
//                     return day_cmp;
//                 }

//                 // Same day: prioritize non all-day tasks before all-day tasks
//                 match (a.is_all_day, b.is_all_day) {
//                     (true, false) => return std::cmp::Ordering::Greater,
//                     (false, true) => return std::cmp::Ordering::Less,
//                     _ => {}
//                 }

//                 // Same day and same all-day status: compare by time
//                 a.due_date.cmp(&b.due_date)
//             }
//         };

//         if due_cmp != std::cmp::Ordering::Equal {
//             return due_cmp;
//         }

//         // If due dates are equal (including time), compare start dates (unset dates go to the end)
//         let start_cmp = match (is_unset(&a.start_date), is_unset(&b.start_date)) {
//             (true, true) => std::cmp::Ordering::Equal,
//             (true, false) => std::cmp::Ordering::Greater,
//             (false, true) => std::cmp::Ordering::Less,
//             (false, false) => {
//                 // First compare by day
//                 let day_cmp = compare_by_day(&a.start_date, &b.start_date);
//                 if day_cmp != std::cmp::Ordering::Equal {
//                     return day_cmp;
//                 }

//                 // Same day: prioritize non all-day tasks before all-day tasks
//                 match (a.is_all_day, b.is_all_day) {
//                     (true, false) => return std::cmp::Ordering::Greater,
//                     (false, true) => return std::cmp::Ordering::Less,
//                     _ => {}
//                 }

//                 // Same day and same all-day status: compare by time
//                 a.start_date.cmp(&b.start_date)
//             }
//         };

//         if start_cmp != std::cmp::Ordering::Equal {
//             return start_cmp;
//         }

//         // If all dates are equal, sort by sort_order
//         a.sort_order.cmp(&b.sort_order)
//     });
// }

// #[derive(Debug, Clone)]
// pub enum RepeatFreq {
//     Daily,
//     Weekly,
//     Monthly,
//     Yearly,
//     Weekdays,
//     // Custom(String),
// }

// #[derive(Debug, Clone)]
// pub struct RepeatFlag {
//     freq: RepeatFreq,
//     interval: u32,
//     days: Option<Vec<Weekday>>,
// }
// impl RepeatFlag {
//     pub fn new(freq: RepeatFreq, interval: u32, days: Option<Vec<Weekday>>) -> Self {
//         Self {
//             freq,
//             interval,
//             days,
//         }
//     }

//     pub fn freq(&self) -> &RepeatFreq {
//         &self.freq
//     }

//     // pub fn interval(&self) -> u32 {
//     //     self.interval
//     // }

//     pub fn days(&self) -> &Option<Vec<Weekday>> {
//         &self.days
//     }

//     /// Parse an RRULE string into a RepeatFlag
//     /// Example: "RRULE:FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,FR"
//     pub fn from_string(rrule: &str) -> Option<Self> {
//         if rrule.is_empty() {
//             return None;
//         }

//         // Remove "RRULE:" prefix if present
//         let rule = rrule.strip_prefix("RRULE:").unwrap_or(rrule);

//         let mut freq = None;
//         let mut interval = 1u32;
//         let mut days = None;

//         // Parse the RRULE components
//         for part in rule.split(';') {
//             if let Some((key, value)) = part.split_once('=') {
//                 match key {
//                     "FREQ" => {
//                         freq = match value {
//                             "DAILY" => Some(RepeatFreq::Daily),
//                             "WEEKLY" => Some(RepeatFreq::Weekly),
//                             "MONTHLY" => Some(RepeatFreq::Monthly),
//                             "YEARLY" => Some(RepeatFreq::Yearly),
//                             _ => None,
//                         };
//                     }
//                     "INTERVAL" => {
//                         interval = value.parse().unwrap_or(1);
//                     }
//                     "BYDAY" => {
//                         let parsed_days: Vec<Weekday> = value
//                             .split(',')
//                             .filter_map(|day| match day {
//                                 "MO" => Some(Weekday::Mon),
//                                 "TU" => Some(Weekday::Tue),
//                                 "WE" => Some(Weekday::Wed),
//                                 "TH" => Some(Weekday::Thu),
//                                 "FR" => Some(Weekday::Fri),
//                                 "SA" => Some(Weekday::Sat),
//                                 "SU" => Some(Weekday::Sun),
//                                 _ => None,
//                             })
//                             .collect();

//                         // Check if this is the weekdays pattern
//                         if parsed_days.len() == 5
//                             && parsed_days.iter().all(|d| {
//                                 matches!(
//                                     d,
//                                     Weekday::Mon
//                                         | Weekday::Tue
//                                         | Weekday::Wed
//                                         | Weekday::Thu
//                                         | Weekday::Fri
//                                 )
//                             })
//                         {
//                             freq = Some(RepeatFreq::Weekdays);
//                         } else if !parsed_days.is_empty() {
//                             days = Some(parsed_days);
//                         }
//                     }
//                     _ => {}
//                 }
//             }
//         }

//         freq.map(|f| Self::new(f, interval, days))
//     }

//     /// Create a human-readable string representation of the repeat pattern
//     pub fn to_pretty_string(&self) -> String {
//         match (&self.freq, self.interval, &self.days) {
//             // Weekdays special case
//             (RepeatFreq::Weekdays, _, _) => " weekdays".to_string(),

//             // Daily patterns
//             (RepeatFreq::Daily, 1, _) => " daily".to_string(),
//             (RepeatFreq::Daily, 2, _) => " every other day".to_string(),
//             (RepeatFreq::Daily, n, _) => format!(" every {} days", n),

//             // Weekly patterns
//             (RepeatFreq::Weekly, 1, None) => " weekly".to_string(),
//             (RepeatFreq::Weekly, 1, Some(days)) if days.len() == 1 => {
//                 format!(
//                     " every {}",
//                     Self::day_to_short_string(&days[0]).to_lowercase()
//                 )
//             }
//             (RepeatFreq::Weekly, 1, Some(days)) => {
//                 let days_str = days
//                     .iter()
//                     .map(|d| Self::day_to_short_string(d).to_lowercase())
//                     .collect::<Vec<_>>()
//                     .join(", ");
//                 format!(" every {}", days_str)
//             }
//             (RepeatFreq::Weekly, n, None) => format!(" every {} weeks", n),
//             (RepeatFreq::Weekly, 2, Some(days)) => {
//                 let days_str = days
//                     .iter()
//                     .map(|d| Self::day_to_short_string(d).to_lowercase())
//                     .collect::<Vec<_>>()
//                     .join(", ");
//                 format!(" every other {}", days_str)
//             }
//             (RepeatFreq::Weekly, n, Some(days)) => {
//                 let days_str = days
//                     .iter()
//                     .map(|d| Self::day_to_short_string(d).to_lowercase())
//                     .collect::<Vec<_>>()
//                     .join(", ");
//                 format!(" every {} weeks on {}", n, days_str)
//             }

//             // Monthly patterns
//             (RepeatFreq::Monthly, 1, _) => " monthly".to_string(),
//             (RepeatFreq::Monthly, 2, _) => " every other month".to_string(),
//             (RepeatFreq::Monthly, n, _) => format!(" every {} months", n),

//             // Yearly patterns
//             (RepeatFreq::Yearly, 1, _) => " yearly".to_string(),
//             (RepeatFreq::Yearly, 2, _) => " every other year".to_string(),
//             (RepeatFreq::Yearly, n, _) => format!(" every {} years", n),
//         }
//     }

//     fn day_to_short_string(day: &Weekday) -> &'static str {
//         match day {
//             Weekday::Mon => "Mon",
//             Weekday::Tue => "Tue",
//             Weekday::Wed => "Wed",
//             Weekday::Thu => "Thu",
//             Weekday::Fri => "Fri",
//             Weekday::Sat => "Sat",
//             Weekday::Sun => "Sun",
//         }
//     }

//     pub fn build(&self) -> String {
//         let mut flag = String::from("RRULE:");
//         let freq_str = match &self.freq {
//             RepeatFreq::Daily => "FREQ=DAILY",
//             RepeatFreq::Weekly => "FREQ=WEEKLY",
//             RepeatFreq::Monthly => "FREQ=MONTHLY",
//             RepeatFreq::Yearly => "FREQ=YEARLY",
//             RepeatFreq::Weekdays => "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR",
//             // RepeatFreq::Custom(s) => s,
//         };
//         flag += freq_str;
//         flag += &format!(";INTERVAL={}", self.interval);
//         if let Some(days) = &self.days {
//             let days_str: Vec<&str> = days
//                 .iter()
//                 .map(|day| match day {
//                     Weekday::Mon => "MO",
//                     Weekday::Tue => "TU",
//                     Weekday::Wed => "WE",
//                     Weekday::Thu => "TH",
//                     Weekday::Fri => "FR",
//                     Weekday::Sat => "SA",
//                     Weekday::Sun => "SU",
//                 })
//                 .collect();
//             flag += &format!(";BYDAY={}", days_str.join(","));
//         }
//         flag
//     }
// }

// /// Helper function to format an optional repeat flag string for display
// pub fn format_repeat_flag(repeat_flag: &Option<String>) -> Option<String> {
//     repeat_flag
//         .as_ref()
//         .and_then(|flag| RepeatFlag::from_string(flag))
//         .map(|rf| rf.to_pretty_string())
// }
