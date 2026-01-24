use gpui::{
    BoxShadow, Context, CursorStyle, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, div, hsla, point, prelude::FluentBuilder,
    px,
};
use gpui_component::{ActiveTheme, h_flex};
use ticks::{
    projects::ProjectID,
    tasks::{TaskID, TaskPriority},
};

use crate::{
    components::drag_drop::{
        DragData, Draggable, DropIndicator, DropPosition, DropZone, DropZoneStyle,
    },
    stores::TaskStore,
    tasks::TaskData,
};

pub struct TestView {
    // focus_handle: FocusHandle,
    todos: Vec<TaskData>,
    in_progress: Vec<TaskData>,
    completed: Vec<TaskData>,
    drop_target: Option<String>,
    // Per-column insertion tracking
    todo_insertion: Option<usize>,
    progress_insertion: Option<usize>,
    completed_insertion: Option<usize>,
    last_active_zone: Option<String>,
}

impl TestView {
    pub fn new(_task_store: Entity<TaskStore>, _cx: &mut Context<Self>) -> Self {
        // let todos = task_store
        //     .read(cx)
        //     .get_all_tasks()
        //     .into_iter()
        //     .cloned()
        //     .collect();
        let todos = vec![
            TaskData {
                title: Some("Design UI Mockups".into()),
                task_id: Some(TaskID("task-001".into())),
                project_id: Some(ProjectID("project-123".into())),
                content: None,
                due_date: None,
                priority: Some(TaskPriority::High),
                repeat_flag: None,
            },
            TaskData {
                title: Some("Implement Authentication".into()),
                task_id: Some(TaskID("task-002".into())),
                project_id: Some(ProjectID("project-123".into())),
                content: None,
                due_date: None,
                priority: Some(TaskPriority::Medium),
                repeat_flag: None,
            },
            TaskData {
                title: Some("Set Up Database".into()),
                task_id: Some(TaskID("task-003".into())),
                project_id: Some(ProjectID("project-123".into())),
                content: None,
                due_date: None,
                priority: Some(TaskPriority::Low),
                repeat_flag: None,
            },
            TaskData {
                title: Some("Write Unit Tests".into()),
                task_id: Some(TaskID("task-004".into())),
                project_id: Some(ProjectID("project-123".into())),
                content: None,
                due_date: None,
                priority: Some(TaskPriority::None),
                repeat_flag: None,
            },
            TaskData {
                title: Some("Deploy to Staging".into()),
                task_id: Some(TaskID("task-005".into())),
                project_id: Some(ProjectID("project-123".into())),
                content: None,
                due_date: None,
                priority: Some(TaskPriority::High),
                repeat_flag: None,
            },
        ];
        Self {
            // focus_handle: cx.focus_handle(),
            todos,
            in_progress: vec![],
            completed: vec![],
            drop_target: None,
            todo_insertion: None,
            progress_insertion: None,
            completed_insertion: None,
            last_active_zone: None,
        }
    }

    /// Handle moving tasks between columns
    fn handle_task_drop(&mut self, task: TaskData, target_zone: &str, cx: &mut Context<Self>) {
        let insertion_index = match target_zone {
            "todo" => self.todo_insertion,
            "progress" => self.progress_insertion,
            "completed" => self.completed_insertion,
            _ => None,
        };
        // Remove task from all lists first
        self.todos.retain(|t| t.task_id != task.task_id);
        self.in_progress.retain(|t| t.task_id != task.task_id);
        self.completed.retain(|t| t.task_id != task.task_id);

        // Add to target list at the specified position
        let target_list = match target_zone {
            "todo" => &mut self.todos,
            "progress" => &mut self.in_progress,
            "completed" => &mut self.completed,
            _ => &mut self.todos, // fallback
        };

        if let Some(index) = insertion_index {
            let insert_at = index.min(target_list.len());
            target_list.insert(insert_at, task);
        } else {
            target_list.push(task);
        }

        self.drop_target = None;
        self.todo_insertion = None;
        self.progress_insertion = None;
        self.completed_insertion = None;
        self.last_active_zone = None;
        cx.notify();
    }

    fn handle_drag_move(
        &mut self,
        zone: &str,
        position: gpui::Point<gpui::Pixels>,
        bounds: gpui::Bounds<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        println!("DEBUG: handle_drag_move called for zone: {}", zone);
        // Calculate which position in the list this corresponds to
        let list_len = match zone {
            "todo" => self.todos.len(),
            "progress" => self.in_progress.len(),
            "completed" => self.completed.len(),
            _ => 0,
        };

        // Calculate insertion index based on actual item positions (top-aligned layout)
        let padding_top = px(16.0);
        let relative_y = position.y - bounds.origin.y - padding_top;

        let insertion_index = if list_len == 0 {
            Some(0)
        } else if relative_y <= px(0.0) {
            // Above the first item
            Some(0)
        } else {
            // Estimate item height: padding (12px * 2) + content (~30px) + gaps (8px) = ~62px
            let estimated_item_height = px(62.0);
            let gap_height = px(8.0);

            // Calculate which item position we're closest to
            let items_total_height =
                estimated_item_height * list_len as f32 + gap_height * (list_len - 1) as f32;

            if relative_y >= items_total_height + gap_height {
                // Beyond all items
                Some(list_len)
            } else {
                // Find which insertion point based on item positions
                let item_with_gap_height = estimated_item_height + gap_height;
                let index =
                    ((relative_y + gap_height / 2.0) / item_with_gap_height).floor() as usize;
                Some(index.min(list_len))
            }
        };

        // Only update if this is a different zone or the first time
        if self.last_active_zone.as_deref() != Some(zone) {
            // Clear all zones when switching zones
            self.todo_insertion = None;
            self.progress_insertion = None;
            self.completed_insertion = None;
            self.last_active_zone = Some(zone.to_string());
        }

        // Set insertion indicator only for the current zone
        match zone {
            "todo" => self.todo_insertion = insertion_index,
            "progress" => self.progress_insertion = insertion_index,
            "completed" => self.completed_insertion = insertion_index,
            _ => {}
        }

        println!(
            "DEBUG: Zone {} - list_len: {}, relative_y: {:.1}, calculated index: {:?}",
            zone,
            list_len,
            relative_y / px(1.0),
            insertion_index
        );
        cx.notify();
    }
}

impl Render for TestView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .p_3()
            .gap(px(24.0))
            .w_full()
            .child(
                // Todo column
                self.render_column(
                    "To Do",
                    &self.todos,
                    "todo",
                    theme.muted,
                    DropZoneStyle::Dashed,
                    cx,
                ),
            )
            .child(
                // In Progress column
                self.render_column(
                    "In Progress",
                    &self.in_progress,
                    "progress",
                    gpui::rgb(0x3b82f6).into(),
                    DropZoneStyle::Solid,
                    cx,
                ),
            )
            .child(
                // Completed column
                self.render_column(
                    "Completed",
                    &self.completed,
                    "completed",
                    gpui::rgb(0x10b981).into(),
                    DropZoneStyle::Filled,
                    cx,
                ),
            )
    }
}

impl TestView {
    fn render_column(
        &self,
        title: &'static str,
        tasks: &[TaskData],
        zone_id: &'static str,
        accent_color: Hsla,
        zone_style: DropZoneStyle,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let is_drop_target = self.drop_target.as_ref() == Some(&zone_id.to_string());

        // Get the insertion index for this specific column
        let insertion_index = match zone_id {
            "todo" => self.todo_insertion,
            "progress" => self.progress_insertion,
            "completed" => self.completed_insertion,
            _ => None,
        };

        let is_drag_over = insertion_index.is_some();
        let insertion_indicator = if is_drag_over {
            insertion_index.map(|index| DropIndicator {
                index,
                position: DropPosition::Before,
            })
        } else {
            None
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(280.0))
            .child(
                // Column header
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mb(px(12.0))
                    .px(px(8.0))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(2.0))
                            .rounded(px(12.0))
                            .bg(accent_color.opacity(0.2))
                            .text_color(accent_color)
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("{}", tasks.len())),
                    ),
            )
            .child(
                // Drop zone with enhanced visual feedback
                DropZone::<DragData<TaskData>>::new(zone_id)
                    .drop_zone_style(zone_style)
                    .active(is_drop_target || is_drag_over)
                    .insertion_indicator(insertion_indicator)
                    .min_h(px(400.0))
                    // Enhanced styling using the Styled trait
                    .border_2()
                    .border_color(if is_drop_target || is_drag_over {
                        accent_color
                    } else {
                        accent_color.opacity(0.3)
                    })
                    .bg(if is_drop_target || is_drag_over {
                        accent_color.opacity(0.15)
                    } else {
                        accent_color.opacity(0.05)
                    })
                    .shadow(vec![BoxShadow {
                        color: accent_color.opacity(0.1),
                        offset: point(px(0.0), px(2.0)),
                        blur_radius: px(8.0),
                        spread_radius: px(0.0),
                    }])
                    // Enhanced drop handling with visual feedback
                    .on_drop(cx.listener(move |this, data: &DragData<TaskData>, _, cx| {
                        this.handle_task_drop(data.data.clone(), zone_id, cx);
                    }))
                    .on_drag_move({
                        let zone_id = zone_id.to_string();
                        cx.listener(
                            move |this,
                                  event: &gpui::DragMoveEvent<DragData<TaskData>>,
                                  _window,
                                  cx| {
                                // Only handle the drag move if this is the intended zone
                                // Use the event's bounds which are relative to the drop zone
                                let position = event.event.position;
                                let bounds = event.bounds;

                                // Only process if mouse is actually within this zone's bounds
                                if position.x >= bounds.origin.x
                                    && position.x <= bounds.origin.x + bounds.size.width
                                    && position.y >= bounds.origin.y
                                    && position.y <= bounds.origin.y + bounds.size.height
                                {
                                    this.handle_drag_move(&zone_id, position, bounds, cx);
                                }
                            },
                        )
                    })
                    // Conditional drop acceptance
                    .can_drop(move |dragged, _, _| {
                        dragged.downcast_ref::<DragData<TaskData>>().is_some()
                    })
                    .when(tasks.is_empty(), |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap(px(8.0))
                                .py(px(32.0))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .text_color(theme.muted_foreground)
                                        .child("Drop tasks here"),
                                ),
                        )
                    })
                    .children(tasks.iter().enumerate().map(|(ix, task)| {
                        let task_clone = task.clone();
                        let task_title_clone =
                            task.title.clone().unwrap_or("<Untitled Task>".into());
                        let bg = theme.popover.opacity(0.95);
                        let border = theme.border;
                        let fg = theme.foreground;
                        let drag_data = DragData::new(task_clone)
                            .with_label(SharedString::from(task_title_clone.clone()))
                            .with_preview(move || {
                                div()
                                    .px(px(12.0))
                                    .py(px(8.0))
                                    .bg(bg)
                                    .border_1()
                                    .border_color(border)
                                    .rounded(px(6.0))
                                    .shadow(vec![BoxShadow {
                                        color: hsla(0.0, 0.0, 0.0, 0.25),
                                        offset: point(px(0.0), px(4.0)),
                                        blur_radius: px(12.0),
                                        spread_radius: px(0.0),
                                    }])
                                    .text_size(px(13.0))
                                    .text_color(fg)
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("Moving: {}", task_title_clone))
                                    .into_any_element()
                            });

                        let (color, label) = match task.priority.unwrap_or_default() {
                            TaskPriority::None => (gpui::rgb(0x9ca3af), "No Priority"),
                            TaskPriority::Low => (gpui::rgb(0x22c55e), "Low Priority"),
                            TaskPriority::Medium => (gpui::rgb(0xf59e0b), "Medium Priority"),
                            TaskPriority::High => (gpui::rgb(0xef4444), "High Priority"),
                        };

                        Draggable::new((zone_id, ix), drag_data)
                            // Enhanced draggable styling
                            .cursor_style(CursorStyle::PointingHand)
                            .rounded(px(8.0))
                            .w_full()
                            .shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.05),
                                offset: point(px(0.0), px(1.0)),
                                blur_radius: px(3.0),
                                spread_radius: px(0.0),
                            }])
                            .hover_bg(theme.muted.opacity(0.2))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .p(px(12.0))
                                    .rounded(px(8.0))
                                    .bg(theme.group_box)
                                    .border_1()
                                    .border_color(theme.border)
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(
                                                task.title
                                                    .clone()
                                                    .unwrap_or("<Untitled Task>".into()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .w(px(8.0))
                                                    .h(px(8.0))
                                                    .rounded(px(4.0))
                                                    .bg(color),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(theme.muted_foreground)
                                                    .child(label),
                                            ),
                                    ),
                            )
                    })),
            )
    }
}
