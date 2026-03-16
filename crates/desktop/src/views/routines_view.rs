use chrono::Duration;
use gpui::prelude::*;
use gpui::{
    App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, Render, Subscription, Window, div, font, px,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    input::{Input, InputState},
    label::Label,
    notification::NotificationType,
    v_flex,
};
use simple_core::{Routine, RoutineStep};
use uuid::Uuid;

use crate::components::popover::popover;
use crate::stores::DatabaseStore;
use crate::stores::database_store::{DatabaseError, RoutinesLoaded};
use crate::views::MainViewMode;

pub struct NavigateFromRoutines {
    pub mode: MainViewMode,
}

pub struct StartRoutineEditor {
    pub routine_id: Option<Uuid>,
}

pub struct RoutinesView {
    database_store: Entity<DatabaseStore>,
    routines: Vec<Routine>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<NavigateFromRoutines> for RoutinesView {}
impl EventEmitter<StartRoutineEditor> for RoutinesView {}

impl RoutinesView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, store, _event: &RoutinesLoaded, cx| {
                this.routines = store.read(cx).routines.clone();
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &database_store,
            |_this, _store, event: &DatabaseError, _cx| {
                eprintln!("RoutinesView database error: {}", event.message);
            },
        ));

        let routines = database_store.read(cx).routines.clone();

        Self {
            database_store,
            routines,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for RoutinesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("back-to-home")
                                    .cursor_pointer()
                                    .icon(IconName::ArrowLeft)
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(|_this, _event, _window, cx| {
                                        cx.emit(NavigateFromRoutines {
                                            mode: MainViewMode::Home,
                                        });
                                    })),
                            )
                            .child(Label::new("Routines").text_2xl().font(font("Georgia"))),
                    )
                    .child(
                        Button::new("new-routine")
                            .cursor_pointer()
                            .icon(IconName::Plus)
                            .label("New Routine")
                            .outline()
                            .on_click(cx.listener(|_this, _event, _window, cx| {
                                cx.emit(StartRoutineEditor { routine_id: None });
                            })),
                    ),
            )
            .child(Divider::horizontal().color(theme.border).w_full())
            .child(
                div()
                    .flex_1()
                    .id("routines-scroll")
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .when(self.routines.is_empty(), |this| {
                                this.child(
                                    v_flex()
                                        .items_center()
                                        .justify_center()
                                        .py_8()
                                        .gap_2()
                                        .child(
                                            Label::new("No routines yet")
                                                .text_color(theme.muted_foreground),
                                        )
                                        .child(
                                            Label::new(
                                                "Create a routine to group steps into a reusable sequence",
                                            )
                                            .text_sm()
                                            .text_color(theme.muted_foreground),
                                        ),
                                )
                            })
                            .children(self.routines.iter().enumerate().map(|(i, routine)| {
                                let routine_id = routine.id;
                                let title = routine.title.clone();
                                let content = routine.content.clone();
                                let step_count = routine.steps.len();
                                let theme_inner = theme.clone();

                                div()
                                    .id(ElementId::NamedInteger("routine-item".into(), i as u64))
                                    .w_full()
                                    .p_3()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme_inner.border)
                                    .bg(theme_inner.background)
                                    .hover(|s| s.bg(theme_inner.list_hover.opacity(0.3)))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |_this, _event, _window, cx| {
                                        cx.emit(StartRoutineEditor {
                                            routine_id: Some(routine_id),
                                        });
                                    }))
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .gap(px(2.0))
                                                    .child(
                                                        Label::new(title)
                                                            .text_base()
                                                            .truncate(),
                                                    )
                                                    .when_some(content, |this, desc| {
                                                        this.child(
                                                            Label::new(desc)
                                                                .text_sm()
                                                                .text_color(
                                                                    theme_inner.muted_foreground,
                                                                )
                                                                .truncate(),
                                                        )
                                                    })
                                                    .child(
                                                        Label::new(format!(
                                                            "{} step{}",
                                                            step_count,
                                                            if step_count == 1 { "" } else { "s" }
                                                        ))
                                                        .text_xs()
                                                        .text_color(theme_inner.muted_foreground),
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_1()
                                                    .flex_shrink_0()
                                                    .child(
                                                        Button::new(ElementId::Name(
                                                            format!("run-routine-{i}").into(),
                                                        ))
                                                        .icon(IconName::Play)
                                                        .label("Run")
                                                        .ghost()
                                                        .small()
                                                        .tooltip("Instantiate steps into queue")
                                                        .cursor_pointer()
                                                        .occlude()
                                                        .on_click(cx.listener(
                                                            move |this, _event, window, cx| {
                                                                this.database_store.update(
                                                                    cx,
                                                                    |store, cx| {
                                                                        store.instantiate_routine(
                                                                            routine_id,
                                                                            cx,
                                                                        );
                                                                    },
                                                                );
                                                                window.push_notification(
                                                                    (
                                                                        NotificationType::Success,
                                                                        "Routine added to queue",
                                                                    ),
                                                                    cx,
                                                                );
                                                            },
                                                        )),
                                                    )
                                                    .child(
                                                        Button::new(ElementId::Name(
                                                            format!("delete-routine-{i}").into(),
                                                        ))
                                                        .icon(IconName::Delete)
                                                        .ghost()
                                                        .small()
                                                        .tooltip("Delete routine")
                                                        .on_click(cx.listener(
                                                            move |this, _event, _window, cx| {
                                                                this.database_store.update(
                                                                    cx,
                                                                    |store, cx| {
                                                                        store.delete_routine(
                                                                            routine_id,
                                                                            cx,
                                                                        );
                                                                    },
                                                                );
                                                            },
                                                        )),
                                                    ),
                                            ),
                                    )
                            })),
                    ),
            )
    }
}

// ── RoutineEditor ──────────────────────────────────────────────────────────────

pub struct RoutineEditor {
    pub focus_handle: FocusHandle,
    database_store: Entity<DatabaseStore>,
    routine_id: Option<Uuid>,
    title_input: Entity<InputState>,
    content_input: Entity<InputState>,
    pending_title: Option<String>,
    pending_content: Option<String>,
    /// Steps as (title, duration_minutes).
    steps: Vec<(String, Option<u32>)>,
    new_step_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl Focusable for RoutineEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl RoutineEditor {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        routine_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &database_store,
            |_this, _store, event: &DatabaseError, _cx| {
                eprintln!("RoutineEditor database error: {}", event.message);
            },
        ));

        let title_input = cx.new(|cx| InputState::new(window, cx).placeholder("Routine title"));
        let content_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Description (optional)"));
        let new_step_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("New step title…"));

        let mut pending_title = None;
        let mut pending_content = None;
        let mut steps: Vec<(String, Option<u32>)> = Vec::new();

        if let Some(id) = routine_id {
            if let Some(routine) = database_store.read(cx).get_routine(id) {
                pending_title = Some(routine.title.clone());
                pending_content = routine.content.clone();
                steps = routine
                    .steps
                    .iter()
                    .map(|s| {
                        let mins = s.duration.map(|d| d.num_minutes().max(0) as u32);
                        (s.title.clone(), mins)
                    })
                    .collect();
            }
        }

        Self {
            focus_handle,
            database_store,
            routine_id,
            title_input,
            content_input,
            pending_title,
            pending_content,
            steps,
            new_step_input,
            _subscriptions: subscriptions,
        }
    }

    fn apply_pending_values(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(title) = self.pending_title.take() {
            self.title_input.update(cx, |input, cx| {
                input.set_value(title, window, cx);
            });
        }
        if let Some(content) = self.pending_content.take() {
            self.content_input.update(cx, |input, cx| {
                input.set_value(content, window, cx);
            });
        }
    }

    fn build_and_save_routine(&mut self, cx: &mut Context<Self>) {
        let title = self.title_input.read(cx).value().to_string();
        if title.trim().is_empty() {
            return;
        }

        let content_text = self.content_input.read(cx).value().to_string();
        let content = if content_text.trim().is_empty() {
            None
        } else {
            Some(content_text)
        };

        let steps: Vec<RoutineStep> = self
            .steps
            .iter()
            .map(|(title, duration_mins)| {
                let mut step = RoutineStep::new(title.clone());
                if let Some(mins) = duration_mins {
                    step = step.with_duration(Duration::minutes(*mins as i64));
                }
                step
            })
            .collect();

        let mut routine = Routine::new(title);
        if let Some(id) = self.routine_id {
            routine.id = id;
        } else {
            self.routine_id = Some(routine.id);
        }
        routine.content = content;
        routine.steps = steps;

        self.database_store.update(cx, |store, cx| {
            store.upsert_routine(routine, cx);
        });
    }

    fn add_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.new_step_input.read(cx).value().to_string();
        if title.trim().is_empty() {
            return;
        }
        self.steps.push((title, Some(15)));
        self.new_step_input.update(cx, |input, cx| {
            input.set_value(String::new(), window, cx);
        });
        self.build_and_save_routine(cx);
    }

    fn remove_step(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.steps.len() {
            self.steps.remove(ix);
            self.build_and_save_routine(cx);
        }
    }

    fn render_steps_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        v_flex()
            .w_full()
            .gap_2()
            .child(
                Label::new("Steps")
                    .text_sm()
                    .text_color(theme.muted_foreground),
            )
            .when(self.steps.is_empty(), |this| {
                this.child(
                    div()
                        .w_full()
                        .py_4()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Label::new("No steps yet")
                                .text_sm()
                                .text_color(theme.muted_foreground),
                        ),
                )
            })
            .children(
                self.steps
                    .iter()
                    .enumerate()
                    .map(|(i, (title, duration_mins))| {
                        let theme_inner = theme.clone();
                        let duration_label = duration_mins
                            .map(|m| format!("{}min", m))
                            .unwrap_or_else(|| "—".to_string());

                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .py_1()
                            .px_2()
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(theme_inner.border)
                            .child(
                                div().w(px(20.0)).flex_shrink_0().child(
                                    Label::new(format!("{}", i + 1))
                                        .text_xs()
                                        .text_color(theme_inner.muted_foreground),
                                ),
                            )
                            .child(
                                Label::new(title.clone())
                                    .text_sm()
                                    .truncate()
                                    .flex_1()
                                    .min_w_0(),
                            )
                            .child(
                                Label::new(duration_label)
                                    .text_xs()
                                    .text_color(theme_inner.muted_foreground),
                            )
                            .child(
                                Button::new(ElementId::Name(format!("remove-step-{i}").into()))
                                    .icon(IconName::Close)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Remove step")
                                    .on_click(cx.listener(move |this, _event, _window, cx| {
                                        this.remove_step(i, cx);
                                    })),
                            )
                    }),
            )
    }

    fn render_add_step_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .child(Input::new(&self.new_step_input).flex_1())
            .child(
                Button::new("add-step-btn")
                    .icon(IconName::Plus)
                    .label("Add Step")
                    .outline()
                    .small()
                    .on_click(cx.listener(|this, _event, window, cx| {
                        this.add_step(window, cx);
                    })),
            )
    }

    fn render_properties_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        v_flex()
            .w_full()
            .gap_3()
            .child(
                Input::new(&self.title_input)
                    .w_full()
                    .text_2xl()
                    .appearance(false),
            )
            .child(Input::new(&self.content_input).w_full().appearance(false))
            .child(Divider::horizontal().color(theme.border).w_full())
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex().w_full().justify_end().gap_2().pt_2().child(
            Button::new("save-routine")
                .label(if self.routine_id.is_some() {
                    "Save"
                } else {
                    "Create Routine"
                })
                .primary()
                .on_click(cx.listener(|this, _event, window, cx| {
                    this.build_and_save_routine(cx);
                    window.dispatch_action(Box::new(crate::components::popover::CloseOverlay), cx);
                })),
        )
    }
}

impl Render for RoutineEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_pending_values(window, cx);

        let theme = cx.theme().clone();

        let inner = v_flex()
            .w(px(560.0))
            .max_h(px(680.0))
            .bg(theme.group_box)
            .text_color(theme.group_box_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .shadow_xl()
            .track_focus(&self.focus_handle)
            .on_any_mouse_down(|_event, _window, cx| {
                cx.stop_propagation();
            })
            .p_4()
            .gap_4()
            .child(self.render_properties_section(cx))
            .child(self.render_steps_section(cx))
            .child(self.render_add_step_section(cx))
            .child(self.render_footer(cx));

        popover(inner, cx)
    }
}
