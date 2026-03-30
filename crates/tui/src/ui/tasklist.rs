use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph},
};
use std::{sync::Arc, time::Instant};
use tachyonfx::{EffectManager, EffectTimer, Interpolation, Motion, fx};
use ticks::tasks::{Task, TaskID, TaskPriority};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::AppAction,
    tasks::{TaskAction, TaskData, format_repeat_flag, is_overdue},
    ui::{
        UIAction,
        multiselect::{MultiSelectList, MultiSelectListItem, MultiSelectListState},
        normalmode::NormalModeAction,
        utils,
        viewselector::View,
    },
};

pub struct TaskList {
    all_tasks: Arc<Vec<Arc<Task>>>,
    shown_tasks: Vec<TaskID>,
    // filtered_indices: Vec<usize>,
    list_left: MultiSelectList<'static>,
    list_right: MultiSelectList<'static>,
    list_state: MultiSelectListState,
    style: Style,
    current_block: Option<Block<'static>>,
    pub tasks_loaded: bool,
    pub task_changed: bool,
    last_area: Option<Rect>,
    effects: EffectManager<()>,
    tx: UnboundedSender<AppAction>,
}

impl TaskList {
    pub fn new(tasks: Arc<Vec<Arc<Task>>>, tx: UnboundedSender<AppAction>) -> Self {
        // let filtered_indices = (0..tasks.len()).collect();
        let list_state = MultiSelectListState::default();
        let current_block = Block::default()
            // .title("Tasks")
            .border_set(BorderType::Rounded.to_border_set())
            .borders(Borders::ALL);
        let list_left = MultiSelectList::default()
            .with_block(current_block.clone())
            .with_highlight_symbol(" ")
            .with_highlight_style(
                Style::new()
                    .bg(Color::Rgb(30, 30, 30))
                    .add_modifier(Modifier::BOLD),
            );
        let list_right = MultiSelectList::default()
            .with_block(current_block.clone())
            .with_highlight_style(
                Style::new()
                    .bg(Color::Rgb(30, 30, 30))
                    .add_modifier(Modifier::BOLD),
            );
        let effects: EffectManager<()> = EffectManager::default();
        Self {
            all_tasks: tasks,
            shown_tasks: vec![],
            // filtered_indices,
            list_state,
            list_left,
            list_right,
            style: Style::default(),
            current_block: Some(current_block),
            tasks_loaded: false,
            task_changed: false,
            last_area: None,
            effects,
            tx,
        }
    }

    pub fn activate(&mut self) {
        if self.shown_tasks.is_empty() {
            self.list_state.select(None);
        } else if self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        }
        self.current_block = Some(
            Block::default()
                // .title("Tasks")
                .border_set(BorderType::Rounded.to_border_set())
                .borders(Borders::ALL),
        );
        self.style = Style::default();
    }

    pub fn deactivate(&mut self) {
        self.current_block = Some(
            Block::default()
                // .title("Tasks")
                .borders(Borders::ALL)
                .border_set(BorderType::Rounded.to_border_set())
                .style(Style::default().add_modifier(Modifier::DIM)),
        );
        self.style = Style::default().add_modifier(Modifier::DIM);
    }

    pub fn is_empty(&self) -> bool {
        self.shown_tasks.is_empty()
    }

    pub fn clear_selection(&mut self) {
        self.list_state.select(None);
        self.list_state.end_visual_selection();
    }

    // pub fn insert_new_task(&mut self) {
    //     let mut items: Vec<MultiSelectListItem> = self
    //         .shown_tasks
    //         .iter()
    //         .filter_map(|task_id| {
    //             self.all_tasks.iter().find_map(|task| {
    //                 if task.get_id() == task_id {
    //                     Some(create_list_item(task))
    //                 } else {
    //                     None
    //                 }
    //             })
    //         })
    //         .collect();
    //     items.insert(0, create_new_item());
    //     self.shown_tasks.insert(0, TaskID("".to_string()));
    //     self.list.set_items(items);
    //     self.list_state.select(Some(0));
    // }

    pub fn set_all_tasks(&mut self, tasks: Arc<Vec<Arc<Task>>>) {
        self.all_tasks = tasks;
    }

    pub fn filter_by_view(&mut self, view: &View) {
        let now = Utc::now();
        let old_selection = self.get_current_task();

        self.shown_tasks = view.get_filtered_task_ids(now, self.all_tasks.as_ref());
        if self.shown_tasks.is_empty() {
            self.list_state.select(None);
        } else if let Some(selected) = self.list_state.selected() {
            if selected >= self.shown_tasks.len() {
                self.list_state.select(Some(self.shown_tasks.len() - 1));
            }
        }

        let new_selection = self.get_current_task();
        if old_selection.as_ref().map(|t| t.get_id()) != new_selection.as_ref().map(|t| t.get_id())
        {
            self.task_changed = true;
        }
        // if let Some(area) = self.list.calculate_effect_area() {
        //     let timer = EffectTimer::from_ms(500, Interpolation::Linear);
        //     // let fx = fx::coalesce(timer);
        //     let c = Color::Rgb(25, 25, 25);
        //     let fx = sweep_in(Motion::UpToDown, 5, 0, c, timer).with_area(area);
        //     self.effects.add_effect(fx);
        // }
        let left_items: Vec<MultiSelectListItem> = self
            .shown_tasks
            .iter()
            .filter_map(|task_id| {
                self.all_tasks.iter().find_map(|task| {
                    if task.get_id() == task_id {
                        Some(create_list_left_item(task))
                    } else {
                        None
                    }
                })
            })
            .collect();
        self.list_left.set_items(left_items);
        let now = Utc::now();
        let right_items: Vec<MultiSelectListItem> = self
            .shown_tasks
            .iter()
            .filter_map(|task_id| {
                self.all_tasks.iter().find_map(|task| {
                    if task.get_id() == task_id {
                        Some(create_list_right_item(now, task))
                    } else {
                        None
                    }
                })
            })
            .collect();
        self.list_right.set_items(right_items);
    }

    pub fn start_animation(&mut self) {
        if let Some(area) = self.last_area {
            let area = if let Some(block) = self.current_block.clone() {
                block.inner(area)
            } else {
                area
            };
            if let Some(effect_area) = self.list_left.calculate_effect_area(area) {
                let timer = EffectTimer::from_ms(300, Interpolation::Linear);
                // let fx = fx::coalesce(timer);
                let c = Color::Rgb(25, 25, 25);
                let fx = fx::sweep_in(Motion::UpToDown, 5, 0, c, timer).with_area(effect_area);
                self.effects.add_effect(fx);
            }
        }
    }

    pub fn remove_task(&mut self, task_id: TaskID) {
        if let Some(task) = self.all_tasks.iter().find(|t| t.get_id() == &task_id) {
            let data = TaskData::default()
                .task_id(task.get_id().clone())
                .project_id(task.project_id.clone());
            let task_action = AppAction::TaskAction(TaskAction::Delete, data);
            let msg =
                Text::from(Line::from("Are you sure you want to delete this task?")).centered();
            let confirm_action = AppAction::UIAction(UIAction::Confirm(msg, Box::new(task_action)));
            let _ = self.tx.send(confirm_action);
            if self.shown_tasks.is_empty() {
                self.list_state.select(None);
            } else if let Some(selected) = self.list_state.selected() {
                if selected >= self.shown_tasks.len() {
                    self.list_state.select(Some(self.shown_tasks.len() - 1));
                }
            }
        }
    }

    pub fn remove_selected_tasks(&mut self) {
        if let Some(curr) = self.list_state.selected() {
            if let Some(start) = self.list_state.visual_start {
                let (s, e) = if curr >= start {
                    (start, curr)
                } else {
                    (curr, start)
                };
                // self.remove_range_inclusive((s, e));
                // let task_actions = self.filtered_indices[s..=e].iter().filter_map(|&idx| {
                //     self.all_tasks.get(idx).map(|task| {
                //         let project_id = task.project_id.clone();
                //         let task_id = task.get_id().clone();
                //         AppAction::TaskAction(project_id, task_id, TaskAction::Delete)
                //     })
                // });
                let task_actions = self.shown_tasks[s..=e].iter().filter_map(|task_id| {
                    self.all_tasks.iter().find_map(|task| {
                        if task.get_id() == task_id {
                            let data = TaskData::from_task(&task);
                            Some(AppAction::TaskAction(TaskAction::Delete, data))
                        } else {
                            None
                        }
                    })
                });
                let confirmation_action = AppAction::UIAction(UIAction::Confirm(
                    Text::from(Line::from(vec![
                        Span::from("Are you sure you want to delete "),
                        Span::from(format!("{} ", e - s + 1))
                            .style(Style::default().fg(Color::Yellow)),
                        Span::from("tasks?"),
                    ])),
                    Box::new(AppAction::MultiAction(
                        task_actions
                            .chain(vec![AppAction::UIAction(UIAction::NormalMode(
                                NormalModeAction::ClearSelection,
                            ))])
                            .collect(),
                    )),
                ));
                let _ = self.tx.send(confirmation_action);
                // self.list_state.select_next();
                // self.list_state.select(Some(s));
                // self.list_state.end_visual_selection();
            } else {
                self.remove_task(self.shown_tasks[curr].clone());
                self.list_state.select(Some(curr));
            }
        }
    }

    pub fn get_current_task(&self) -> Option<Arc<Task>> {
        self.list_state.selected().and_then(|selected_idx| {
            self.shown_tasks.get(selected_idx).and_then(|task_id| {
                self.all_tasks.iter().find_map(|task| {
                    if task.get_id() == task_id {
                        Some(Arc::clone(task))
                    } else {
                        None
                    }
                })
            })
        })
    }

    pub fn get_selected_tasks(&self) -> Vec<Arc<Task>> {
        if let Some(curr) = self.list_state.selected() {
            if let Some(start) = self.list_state.visual_start {
                // Visual selection mode - return range of tasks
                let (s, e) = if curr >= start {
                    (start, curr)
                } else {
                    (curr, start)
                };
                let mut selected_tasks = Vec::new();
                for i in s..=e {
                    if let Some(task_id) = self.shown_tasks.get(i) {
                        if let Some(task) = self.all_tasks.iter().find(|t| t.get_id() == task_id) {
                            selected_tasks.push(Arc::clone(task));
                        }
                    }
                }
                selected_tasks
            } else {
                // Single selection mode - return just the current task
                if let Some(task) = self.get_current_task() {
                    vec![task]
                } else {
                    vec![]
                }
            }
        } else {
            vec![]
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        // let idx = self.list_state.selected();
        let task_before = self.get_current_task();
        if self.list_state.is_in_visual_mode() {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => self.list_state.select_next(),
                KeyCode::Char('k') | KeyCode::Up => self.list_state.select_previous(),
                KeyCode::Char('g') => self.list_state.select_first(),
                KeyCode::Char('G') => self.list_state.select_last(),
                KeyCode::Char('d') => self.remove_selected_tasks(),
                KeyCode::Esc => self.list_state.end_visual_selection(),
                _ => {}
            }
            return;
        } else {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => self.list_state.select_next(),
                KeyCode::Char('k') | KeyCode::Up => self.list_state.select_previous(),
                KeyCode::Char('g') => self.list_state.select_first(),
                KeyCode::Char('G') => self.list_state.select_last(),
                KeyCode::Char('v') | KeyCode::Char('V') => self.list_state.start_visual_selection(),
                KeyCode::Char('d') => self.remove_selected_tasks(),
                _ => {}
            }
        }
        if let Some(task_after) = self.get_current_task() {
            if let Some(task_before) = task_before {
                self.task_changed = task_before.get_id() != task_after.get_id();
            }
        }
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        if self.shown_tasks.len() == 0 {
            let msg = if !self.tasks_loaded {
                "\nLoading tasks..."
            } else {
                "\nNo tasks available"
            };
            let mut p = Paragraph::new(msg)
                .style(self.style)
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .title("No projects")
                        // .border_set(BorderType::Rounded.to_border_set())
                        .borders(ratatui::widgets::Borders::ALL),
                );
            if let Some(block) = self.current_block.clone() {
                p = p.block(block);
            }
            f.render_widget(p, area);
            return;
        }

        // let items: Vec<MultiSelectListItem> = self
        //     .shown_tasks
        //     .iter()
        //     .filter_map(|task_id| {
        //         self.all_tasks.iter().find_map(|task| {
        //             if task.get_id() == task_id {
        //                 Some(create_list_item(task))
        //             } else {
        //                 None
        //             }
        //         })
        //     })
        //     .collect();
        // let mut task_list = MultiSelectList::new(items)
        //     .with_style(self.style)
        //     // .with_highlight_symbol(" ● ")
        //     .with_highlight_symbol(" ")
        //     .with_highlight_style(
        //         Style::new()
        //             .bg(Color::Rgb(30, 30, 30))
        //             .add_modifier(Modifier::BOLD),
        //     );
        // task_list.
        // let mut effect_area = area;
        // if let Some(block) = self.current_block.clone() {
        //     // effect_area = block.inner(area);
        //     task_list = task_list.with_block(block);
        // }
        // let effect_area = task_list.calculate_effect_area(area);
        if let Some(block) = self.current_block.clone() {
            self.list_left.set_block(
                block
                    .clone()
                    .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM),
            );
            self.list_right
                .set_block(block.borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM));
        }
        let split_area = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).split(area);
        f.render_stateful_widget(&self.list_right, split_area[1], &mut self.list_state);
        f.render_stateful_widget(&self.list_left, split_area[0], &mut self.list_state);
        let elapsed = last_frame.elapsed();
        self.effects
            .process_effects(elapsed.into(), f.buffer_mut(), area);
        self.last_area = Some(area);
    }
}

fn create_list_left_item(task: &Arc<Task>) -> MultiSelectListItem<'static> {
    // let line1 = Line::from("");
    let mut spans = vec![Span::from(task.title.clone()).style(Style::default())];
    if let Some(priority_syle) = match task.priority {
        TaskPriority::High => Style::default().fg(Color::Red).into(),
        TaskPriority::Medium => Style::default().fg(Color::Yellow).into(),
        TaskPriority::Low => Style::default().fg(Color::Blue).into(),
        TaskPriority::None => None,
    } {
        let priority_symbol = Span::from("● ").style(priority_syle);
        spans.insert(0, priority_symbol);
    }
    let line2 = Line::from(spans);
    // let line3 = Line::from("");
    // MultiSelectListItem::new(vec![line1, line2, line3])
    MultiSelectListItem::new(line2)
}

fn create_list_right_item(now: DateTime<Utc>, task: &Arc<Task>) -> MultiSelectListItem<'static> {
    // let line1 = Line::from("");
    let line2 = if task.due_date.timestamp() > 0 {
        let datetime_str = utils::format_datetime(task.due_date, task.is_all_day);
        let mut spans = vec![Span::from(datetime_str)];

        // Add repeat flag if present
        let repeat_flag = if !task.repeat_flag.is_empty() {
            Some(task.repeat_flag.clone())
        } else {
            None
        };
        if let Some(repeat_str) = format_repeat_flag(&repeat_flag) {
            spans.push(Span::from(" • "));
            spans.push(Span::from(repeat_str));
        }
        spans.push(Span::from(" "));

        let mut line = Line::from(spans);
        if is_overdue(now, task) {
            line = line.style(Style::default().fg(Color::Red).dim());
        } else {
            line = line.style(Style::default().dim());
        }
        line.right_aligned()
    } else {
        Line::from(" ").right_aligned()
    };
    // let line3 = Line::from("");
    // MultiSelectListItem::new(vec![line1, line2, line3])
    MultiSelectListItem::new(line2)
}
