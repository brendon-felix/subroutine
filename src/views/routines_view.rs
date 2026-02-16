use gpui::prelude::*;
use gpui::{
    App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, Render, SharedString, Subscription, Window, div, font, px,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    input::{Input, InputState},
    label::Label,
    notification::NotificationType,
    switch::Switch,
    v_flex,
};

use crate::stores::DatabaseStore;
use crate::stores::database_store::{DatabaseError, RoutineStepsLoaded, RoutinesLoaded};
use crate::views::MainViewMode;
use database::{Routine, RoutineStep};

pub struct NavigateFromRoutines {
    pub mode: MainViewMode,
}

pub struct StartRoutineEditor {
    pub routine_id: Option<String>,
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
                this.routines = store.read(cx).get_routines().clone();
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &database_store,
            |_this, _store, event: &DatabaseError, _cx| {
                eprintln!("RoutinesView: Database error: {}", event.message);
            },
        ));

        let routines = database_store.read(cx).get_routines().clone();

        database_store.update(cx, |store, cx| {
            store.load_routines(cx);
        });

        Self {
            database_store,
            routines,
            _subscriptions: subscriptions,
        }
    }

    fn mode_label(routine: &Routine) -> &'static str {
        if routine.is_sequential {
            "Sequential"
        } else {
            "Parallel"
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
                                    .icon(IconName::ArrowLeft)
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(|_this, _event, _window, cx| {
                                        cx.emit(NavigateFromRoutines {
                                            mode: MainViewMode::Home,
                                        });
                                    })),
                            )
                            .child(
                                Label::new("Routines")
                                    .text_2xl()
                                    .font(font("Georgia")),
                            ),
                    )
                    .child(
                        Button::new("new-routine")
                            .icon(IconName::Plus)
                            .label("New Routine")
                            .outline()
                            .on_click(cx.listener(|_this, _event, _window, cx| {
                                cx.emit(StartRoutineEditor { routine_id: None });
                            })),
                    ),
            )
            .child(
                Divider::horizontal().color(theme.border).w_full(),
            )
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
                                            Label::new("Create a routine to group actions into reusable sequences")
                                                .text_sm()
                                                .text_color(theme.muted_foreground),
                                        ),
                                )
                            })
                            .children(
                                self.routines
                                    .iter()
                                    .enumerate()
                                    .map(|(i, routine)| {
                                        let routine_id = routine.id.clone();
                                        let routine_id_start = routine.id.clone();
                                        let routine_id_delete = routine.id.clone();
                                        let routine_id_edit = routine.id.clone();
                                        let name = routine.name.clone();
                                        let description = routine.description.clone();
                                        let mode_label = Self::mode_label(routine);
                                        let randomizable = routine.allow_randomization;

                                        let mut tags = vec![mode_label];
                                        if randomizable {
                                            tags.push("Randomizable");
                                        }
                                        let tag_text = tags.join(" · ");

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
                                                    routine_id: Some(routine_id_edit.clone()),
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
                                                                Label::new(name)
                                                                    .text_base()
                                                                    .truncate(),
                                                            )
                                                            .when_some(description, |this, desc| {
                                                                this.child(
                                                                    Label::new(desc)
                                                                        .text_sm()
                                                                        .text_color(theme_inner.muted_foreground)
                                                                        .truncate(),
                                                                )
                                                            })
                                                            .child(
                                                                Label::new(tag_text)
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
                                                                    format!("start-routine-{i}").into(),
                                                                ))
                                                                .icon(IconName::Play)
                                                                .label("Start")
                                                                .ghost()
                                                                .small()
                                                                .tooltip("Instantiate into pipeline")
                                                                .on_click(cx.listener(
                                                                    move |this, _event, window, cx| {
                                                                        this.database_store.update(
                                                                            cx,
                                                                            |store, cx| {
                                                                                store.start_routine(
                                                                                    routine_id_start.clone(),
                                                                                    cx,
                                                                                );
                                                                            },
                                                                        );
                                                                        window.push_notification(
                                                                            (
                                                                                NotificationType::Success,
                                                                                "Routine added to pipeline",
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
                                                                                    routine_id_delete.clone(),
                                                                                    cx,
                                                                                );
                                                                            },
                                                                        );
                                                                    },
                                                                )),
                                                            ),
                                                    ),
                                            )
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                    ),
            )
    }
}

pub struct RoutineEditor {
    pub focus_handle: FocusHandle,
    database_store: Entity<DatabaseStore>,
    routine_id: Option<String>,
    name_input: Entity<InputState>,
    description_input: Entity<InputState>,
    pending_name: Option<String>,
    pending_description: Option<String>,
    is_sequential: bool,
    allow_randomization: bool,
    steps: Vec<RoutineStep>,
    available_actions: Vec<(String, String)>,
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
        routine_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, store, _event: &RoutineStepsLoaded, cx| {
                this.steps = store.read(cx).get_routine_steps().clone();
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, store, _event: &RoutinesLoaded, cx| {
                if let Some(ref routine_id) = this.routine_id {
                    if let Some(routine) = store.read(cx).get_routine(routine_id) {
                        this.is_sequential = routine.is_sequential;
                        this.allow_randomization = routine.allow_randomization;
                    }
                }
                cx.notify();
            },
        ));

        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Routine name"));
        let description_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Description (optional)"));

        let mut pending_name = None;
        let mut pending_description = None;
        let mut is_sequential = true;
        let mut allow_randomization = false;
        let mut steps = Vec::new();

        let available_actions: Vec<(String, String)> = database_store
            .read(cx)
            .get_all_actions()
            .iter()
            .map(|action| (action.id.clone(), action.title.clone()))
            .collect();

        if let Some(ref id) = routine_id {
            if let Some(routine) = database_store.read(cx).get_routine(id) {
                pending_name = Some(routine.name.clone());
                pending_description = routine.description.clone();
                is_sequential = routine.is_sequential;
                allow_randomization = routine.allow_randomization;
            }
            database_store.update(cx, |store, cx| {
                store.load_routine_steps(id.clone(), cx);
            });
            steps = database_store.read(cx).get_routine_steps().clone();
        }

        Self {
            focus_handle,
            database_store,
            routine_id,
            name_input,
            description_input,
            pending_name,
            pending_description,
            is_sequential,
            allow_randomization,
            steps,
            available_actions,
            _subscriptions: subscriptions,
        }
    }

    fn apply_pending_values(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(name) = self.pending_name.take() {
            self.name_input.update(cx, |input, cx| {
                input.set_value(&name, window, cx);
            });
        }
        if let Some(description) = self.pending_description.take() {
            self.description_input.update(cx, |input, cx| {
                input.set_value(&description, window, cx);
            });
        }
    }

    fn save_new_routine(&self, cx: &mut Context<Self>) {
        let name: String = self.name_input.read(cx).value().to_string();
        if name.trim().is_empty() {
            return;
        }

        let description_text = self.description_input.read(cx).value().to_string();
        let description = if description_text.trim().is_empty() {
            None
        } else {
            Some(description_text)
        };

        let mut routine = Routine::new(name);
        if let Some(desc) = description {
            routine = routine.description(desc);
        }
        routine = routine
            .is_sequential(self.is_sequential)
            .allow_randomization(self.allow_randomization);

        self.database_store.update(cx, |store, cx| {
            store.create_routine(routine, cx);
        });
    }

    fn render_steps_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        v_flex()
            .w_full()
            .gap_2()
            .child(
                h_flex().w_full().items_center().justify_between().child(
                    Label::new("Steps")
                        .text_sm()
                        .text_color(theme.muted_foreground),
                ),
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
                            Label::new("No steps yet — add actions below")
                                .text_sm()
                                .text_color(theme.muted_foreground),
                        ),
                )
            })
            .children(
                self.steps
                    .iter()
                    .enumerate()
                    .map(|(i, step)| {
                        let step_id = step.id.clone();
                        let routine_id = step.routine_id.clone();
                        let title = step
                            .action_title
                            .clone()
                            .unwrap_or_else(|| format!("Action {}", step.action_id));
                        let order = step.step_order;

                        let theme_inner = theme.clone();

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
                                div().w(px(24.0)).flex_shrink_0().child(
                                    Label::new(format!("{}", order + 1))
                                        .text_xs()
                                        .text_color(theme_inner.muted_foreground),
                                ),
                            )
                            .child(Label::new(title).text_sm().truncate().flex_1().min_w_0())
                            .child(
                                Button::new(ElementId::Name(format!("remove-step-{i}").into()))
                                    .icon(IconName::Close)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Remove step")
                                    .on_click(cx.listener(move |this, _event, _window, cx| {
                                        this.database_store.update(cx, |store, cx| {
                                            store.remove_routine_step(
                                                step_id.clone(),
                                                routine_id.clone(),
                                                cx,
                                            );
                                        });
                                    })),
                            )
                    })
                    .collect::<Vec<_>>(),
            )
    }

    fn render_add_step_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let routine_id = self.routine_id.clone();

        v_flex()
            .w_full()
            .gap_1()
            .child(
                Label::new("Add Action as Step")
                    .text_sm()
                    .text_color(theme.muted_foreground),
            )
            .when(self.available_actions.is_empty(), |this| {
                this.child(
                    Label::new("No actions available — create some actions first")
                        .text_xs()
                        .text_color(theme.muted_foreground),
                )
            })
            .when_some(routine_id, |container, routine_id| {
                container.child(
                    div()
                        .id("add-step-actions-scroll")
                        .overflow_y_scroll()
                        .max_h(px(160.0))
                        .w_full()
                        .child(
                            v_flex().w_full().gap_1().children(
                                self.available_actions
                                    .iter()
                                    .enumerate()
                                    .map(|(i, (action_id, action_title))| {
                                        let action_id_clone = action_id.clone();
                                        let routine_id_clone = routine_id.clone();
                                        let title = action_title.clone();

                                        Button::new(ElementId::Name(
                                            format!("add-action-step-{i}").into(),
                                        ))
                                        .label(title)
                                        .ghost()
                                        .small()
                                        .w_full()
                                        .on_click(
                                            cx.listener(move |this_editor, _event, _window, cx| {
                                                this_editor.database_store.update(
                                                    cx,
                                                    |store, cx| {
                                                        store.add_routine_step(
                                                            routine_id_clone.clone(),
                                                            action_id_clone.clone(),
                                                            cx,
                                                        );
                                                    },
                                                );
                                            }),
                                        )
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                        ),
                )
            })
    }

    fn render_properties_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        v_flex()
            .w_full()
            .gap_3()
            .child(
                Input::new(&self.name_input)
                    .w_full()
                    .text_2xl()
                    .appearance(false),
            )
            .child(
                Input::new(&self.description_input)
                    .w_full()
                    .appearance(false),
            )
            .child(Divider::horizontal().color(theme.border).w_full())
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        v_flex().child(Label::new("Sequential").text_sm()).child(
                            Label::new("Steps run in order")
                                .text_xs()
                                .text_color(theme.muted_foreground),
                        ),
                    )
                    .child(
                        Switch::new("sequential-toggle")
                            .checked(self.is_sequential)
                            .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                                this.is_sequential = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        v_flex().child(Label::new("Randomizable").text_sm()).child(
                            Label::new("Allow random step order")
                                .text_xs()
                                .text_color(theme.muted_foreground),
                        ),
                    )
                    .child(
                        Switch::new("randomizable-toggle")
                            .checked(self.allow_randomization)
                            .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                                this.allow_randomization = *checked;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_new = self.routine_id.is_none();

        h_flex()
            .w_full()
            .justify_end()
            .gap_2()
            .pt_2()
            .when(is_new, |this| {
                this.child(
                    Button::new("save-routine")
                        .label("Create Routine")
                        .primary()
                        .on_click(cx.listener(|this, _event, window, cx| {
                            this.save_new_routine(cx);
                            window.dispatch_action(
                                Box::new(crate::components::popover::CloseOverlay),
                                cx,
                            );
                        })),
                )
            })
            .when(!is_new, |this| {
                this.child(
                    Button::new("close-editor")
                        .label("Done")
                        .primary()
                        .on_click(|_event, window, cx| {
                            window.dispatch_action(
                                Box::new(crate::components::popover::CloseOverlay),
                                cx,
                            );
                        }),
                )
            })
    }
}

impl Render for RoutineEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_pending_values(window, cx);
        let theme = cx.theme();
        let is_editing = self.routine_id.is_some();

        let heading: SharedString = if is_editing {
            "Edit Routine".into()
        } else {
            "New Routine".into()
        };

        let inner = v_flex().h_full().w(px(600.0)).pt_8().child(
            v_flex()
                .track_focus(&self.focus_handle)
                .min_h(px(400.0))
                .max_h(px(700.0))
                .bg(theme.group_box)
                .text_color(theme.group_box_foreground)
                .border_1()
                .border_color(theme.border)
                .rounded_lg()
                .shadow_xl()
                .on_any_mouse_down(|_event, _window, cx| {
                    cx.stop_propagation();
                })
                .child(
                    v_flex()
                        .size_full()
                        .occlude()
                        .py_6()
                        .px_6()
                        .gap_4()
                        .child(
                            Label::new(heading)
                                .text_sm()
                                .text_color(theme.muted_foreground),
                        )
                        .child(self.render_properties_section(cx))
                        .when(is_editing, |this| {
                            this.child(Divider::horizontal().color(cx.theme().border).w_full())
                                .child(
                                    div()
                                        .flex_1()
                                        .id("routine-editor-scroll")
                                        .overflow_y_scroll()
                                        .child(
                                            v_flex()
                                                .gap_4()
                                                .child(self.render_steps_section(cx))
                                                .child(self.render_add_step_section(cx)),
                                        ),
                                )
                        })
                        .child(self.render_footer(cx)),
                ),
        );

        crate::components::popover::popover(inner, cx)
    }
}
