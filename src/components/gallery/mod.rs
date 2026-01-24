use std::{ops::Range, time::Duration};

use gpui::{
    App, Axis, ClickEvent, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyBinding, ParentElement, Pixels, Render, RenderOnce, Size,
    StatefulInteractiveElement, Styled, Window, actions, div, prelude::FluentBuilder, px, size,
};
use gpui_component::Selectable;
use gpui_transitions::{Transition, WindowUseTransition};

mod delegate;
mod item;
mod layout;

pub use delegate::*;
pub use layout::*;

pub fn reverse(t: f32) -> f32 {
    1.0 - t
}

/// Ease in and out with quadratic function
pub fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        let x = -2.0 * t + 2.0;
        1.0 - x * x / 2.0
    }
}

actions!(
    gallery,
    [
        // Cancel,
        NavigateUp,
        NavigateDown,
        NavigateLeft,
        NavigateRight,
        // SelectFirst,
        // SelectLast
        PauseAnimations,
        ResumeAnimations,
        StopAnimations,
    ]
);

pub fn init(cx: &mut App) {
    let context: Option<&str> = Some("Gallery");
    cx.bind_keys([
        // KeyBinding::new("escape", Cancel, context),
        // KeyBinding::new("enter", Confirm { secondary: false }, context),
        // KeyBinding::new("secondary-enter", Confirm { secondary: true }, context),
        KeyBinding::new("up", NavigateUp, context),
        KeyBinding::new("k", NavigateUp, context),
        KeyBinding::new("down", NavigateDown, context),
        KeyBinding::new("j", NavigateDown, context),
        KeyBinding::new("left", NavigateLeft, context),
        KeyBinding::new("h", NavigateLeft, context),
        KeyBinding::new("right", NavigateRight, context),
        KeyBinding::new("l", NavigateRight, context),
        // KeyBinding::new("home", SelectFirst, context),
        // KeyBinding::new("g", SelectFirst, context),
        // KeyBinding::new("end", SelectLast, context),
        // KeyBinding::new("G", SelectLast, context),
        KeyBinding::new("space", PauseAnimations, context),
        KeyBinding::new("r", ResumeAnimations, context),
        KeyBinding::new("s", StopAnimations, context),
    ]);
}

// #[derive(Clone, Copy, PartialEq, Eq, Debug)]
// pub enum GalleryAnimationState {
//     MoveRight,
//     MoveLeft,
//     Inactive,
// }

// #[derive(Clone, Copy, PartialEq, Eq, Debug)]
// pub enum GalleryAnimationMode {
//     Slide,
//     Fade,
//     Scale,
//     Combined,
// }

// impl Default for GalleryAnimationMode {
//     fn default() -> Self {
//         Self::Slide
//     }
// }

#[derive(Clone, Copy)]
pub struct GalleryOptions {
    pub axis: Axis,
    pub max_item_size: Size<Pixels>,
    // pub focused_item_scale: f32,
    // pub max_viewable_items: usize,
    // pub animation_mode: GalleryAnimationMode,
    pub transition_duration: Duration,
    // pub gap: Pixels,
}

impl Default for GalleryOptions {
    fn default() -> Self {
        Self {
            axis: Axis::Horizontal,
            max_item_size: size(px(200.0), px(300.0)),
            // focused_item_scale: 1.5,
            // max_viewable_items: 7,
            // animation_mode: GalleryAnimationMode::Slide,
            transition_duration: Duration::from_millis(500),
            // gap: px(15.0),
        }
    }
}

#[derive(Clone)]
pub struct GalleryTransitions {
    pub slide: Transition<f32>,
    pub fade_first: Transition<f32>,
    pub fade_last: Transition<f32>,
}

pub struct GalleryState<D: GalleryDelegate> {
    pub focus_handle: FocusHandle,
    options: GalleryOptions,
    delegate: D,
    num_entries: usize,
    focused_index: usize,
    // items: Vec<GalleryItem>,
    // transitions: Vec<(usize, Transition<f32>)>,
    transitions: Option<GalleryTransitions>, // must be initialized in the render method
}

impl<D> GalleryState<D>
where
    D: GalleryDelegate,
{
    pub fn new(
        delegate: D,
        options: GalleryOptions,
        // window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // let items = vec![
        //     GalleryItem::new(
        //         ElementId::Name("gallery-item-0".into()),
        //         // "First Item".to_string(),
        //     ),
        //     GalleryItem::new(
        //         ElementId::Name("gallery-item-1".into()),
        //         // "Another Item".to_string(),
        //     ),
        //     GalleryItem::new(
        //         ElementId::Name("gallery-item-2".into()),
        //         // "Last Item".to_string(),
        //     ),
        // ];

        Self {
            focus_handle: cx.focus_handle(),
            options,
            delegate,
            focused_index: 0,
            num_entries: 0,
            // items,
            // transitions: vec![],
            transitions: None,
        }
    }

    // pub fn options(&self) -> &GalleryOptions {
    //     &self.options
    // }

    pub fn delegate_mut(&mut self) -> &mut D {
        &mut self.delegate
    }

    // pub fn set_animation_mode(&mut self, mode: GalleryAnimationMode) {
    //     self.options.animation_mode = mode;
    // }

    fn prepare_items_if_needed(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.num_entries = self.delegate.items_count(cx)
    }

    pub fn set_focused_index(&mut self, index: usize) {
        self.focused_index = index;
        // self.delegate.set_focused_index(index);
    }

    fn render_gallery_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + Styled {
        let is_focused = self.focused_index == ix;
        div()
            .size_full()
            .children(self.delegate.render_item(ix, window, cx).map(|item| {
                item.size_full()
                    .flex()
                    .items_center()
                    .selected(is_focused)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, _cx| {
                        this.focused_index = ix;
                        // this.confirm(
                        //     &Confirm {
                        //         secondary: e.modifiers().secondary(),
                        //     },
                        //     window,
                        //     cx,
                        // );
                    }))
            }))
    }

    pub fn render_items(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + InteractiveElement {
        // let theme = cx.theme().clone();

        // self.transitions.clear();

        // let num_visible = 7;
        // let num_before = num_visible / 2;
        // let num_after = num_visible - num_before;

        // let focused_ix = self.focused_index;
        // let start_ix = focused_ix.saturating_sub(num_before);
        // let end_ix = cmp::min(focused_ix + num_after, self.num_entries);
        // let visible_range = start_ix..end_ix;

        // for ix in visible_range {
        //     // let base_position = match self.options.axis {
        //     //     Axis::Horizontal => point(
        //     //         (self.options.max_item_size.width + self.options.gap) * i,
        //     //         px(0.0),
        //     //     ),
        //     //     Axis::Vertical => point(
        //     //         px(0.0),
        //     //         (self.options.max_item_size.height + self.options.gap) * i,
        //     //     ),
        //     // };

        //     let gallery_pos = ix as i32 - focused_ix as i32;

        //     let transition = window
        //         .use_keyed_transition(
        //             ElementId::NamedChild(
        //                 Box::new(ElementId::Name("gallery".into())),
        //                 format!("item-transition-{}", i).into(),
        //             ),
        //             cx,
        //             self.options.animation_duration,
        //             move |_window, _cx| gallery_pos as f32,
        //         )
        //         .with_easing(|t| reverse(ease_in_out(t)));

        //     self.transitions.push(transition);
        // }

        // self.transitions = visible_range
        //     .map(|ix| {
        //         let gallery_pos = ix as i32 - focused_ix as i32;
        //         (
        //             ix,
        //             window
        //                 .use_keyed_transition(
        //                     ElementId::NamedChild(
        //                         Box::new(ElementId::Name("gallery".into())),
        //                         format!("item-transition-{}", ix).into(),
        //                     ),
        //                     cx,
        //                     self.options.animation_duration,
        //                     move |_window, _cx| gallery_pos as f32,
        //                 )
        //                 .with_easing(|t| reverse(ease_in_out(t))),
        //         )
        //     })
        //     .collect();

        let slide_transition = Some(
            window
                .use_keyed_transition(
                    "gallery-item-slide",
                    cx,
                    self.options.transition_duration,
                    move |_window, _cx| 0., // position starts at 1 and returns to 0
                )
                .continuous(true)
                .with_easing(|t| reverse(ease_in_out(t))),
        );
        let fade_in_transition = Some(
            window
                .use_keyed_transition(
                    "gallery-item-fade-in",
                    cx,
                    self.options.transition_duration,
                    move |_window, _cx| 0., // opacity starts at 0 and ends at 1
                )
                .continuous(true)
                .with_easing(ease_in_out),
        );
        let fade_out_transition = Some(
            window
                .use_keyed_transition(
                    "gallery-item-fade-out",
                    cx,
                    self.options.transition_duration,
                    move |_window, _cx| 1., // opacity starts at 1 and ends at 0
                )
                .continuous(true)
                .with_easing(ease_in_out),
        );

        self.transitions = Some(GalleryTransitions {
            slide: slide_transition.unwrap(),
            fade_first: fade_in_transition.unwrap(),
            fade_last: fade_out_transition.unwrap(),
        });

        div()
            .size_full()
            .relative()
            .when(self.num_entries == 0, |this| {
                this.child(self.delegate.render_empty(window, cx))
            })
            .when(self.num_entries > 0, {
                |this| {
                    this.child(
                        // gallery(
                        //     cx.entity(),
                        //     "gallery",
                        //     self.options.axis,
                        //     self.num_entries,
                        //     size(px(200.0), px(150.0)),
                        //     move |this, visible_range: Range<usize>, window, cx| {
                        //         visible_range
                        //             .map(|ix| {
                        //                 this.render_gallery_item(ix, window, cx).into_any_element()
                        //             })
                        //             .collect::<Vec<_>>()
                        //     },
                        // )
                        gallery(
                            cx.entity(),
                            "gallery",
                            self.options.axis,
                            self.num_entries,
                            self.options.max_item_size,
                            move |this, visible_range: Range<usize>, window, cx| {
                                visible_range
                                    .map(|ix| {
                                        this.render_gallery_item(ix, window, cx).into_any_element()
                                    })
                                    .collect::<Vec<_>>()
                            },
                        )
                        .focused_index(self.focused_index)
                        .transitions(
                            self.transitions
                                .clone()
                                .expect("Slide transition should be initialized"),
                        )
                        // .item_transitions(self.transitions.clone())
                        .gap_8()
                        .track_focus(&self.focus_handle)
                        .on_action(cx.listener(|this, _: &NavigateLeft, _window, cx| {
                            if this.options.axis == Axis::Vertical {
                                return;
                            }
                            // println!("using axis: {:?}", this.options.axis);
                            if this.focused_index > 0 {
                                let new_index = this.focused_index - 1;
                                this.focused_index = new_index;
                                // this.delegate.set_focused_index(Some(new_index));
                                this.transitions.as_mut().map(|transitions| {
                                    transitions.slide.reset(cx);
                                    transitions.slide.update(cx, |pos, _cx| {
                                        *pos = -1.0;
                                    });
                                    transitions.fade_first.reset(cx);
                                    transitions.fade_first.update(cx, |pos, _cx| {
                                        *pos = 1.0;
                                    });
                                    transitions.fade_last.reset(cx);
                                    transitions.fade_last.update(cx, |pos, _cx| {
                                        *pos = 1.0;
                                    });
                                });
                            }
                            cx.notify();
                        }))
                        .on_action(cx.listener(|this, _: &NavigateRight, _window, cx| {
                            // println!("NavigateRight action received");
                            if this.options.axis == Axis::Vertical {
                                return;
                            }
                            // println!("using axis: {:?}", this.options.axis);
                            if this.focused_index < this.num_entries.saturating_sub(1) {
                                let new_index = this.focused_index + 1;
                                this.focused_index = new_index;
                                // this.delegate.set_focused_index(Some(new_index));
                                this.transitions.as_mut().map(|transitions| {
                                    transitions.slide.reset(cx);
                                    transitions.slide.update(cx, |pos, _cx| {
                                        *pos = 1.0;
                                    });
                                    transitions.fade_first.reset(cx);
                                    transitions.fade_first.update(cx, |pos, _cx| {
                                        *pos = 1.0;
                                    });
                                    transitions.fade_last.reset(cx);
                                    transitions.fade_last.update(cx, |pos, _cx| {
                                        *pos = 1.0;
                                    });
                                });

                                cx.notify();
                            }
                        }))
                        .on_action(cx.listener(|this, _: &NavigateUp, _window, cx| {
                            if this.options.axis == Axis::Horizontal {
                                return;
                            }
                            // println!("using axis: {:?}", this.options.axis);
                            if this.focused_index > 0 {
                                let new_index = this.focused_index - 1;
                                this.focused_index = new_index;
                                // this.delegate.set_focused_index(Some(new_index));
                                this.transitions.as_mut().map(|transitions| {
                                    transitions.slide.reset(cx);
                                    transitions.slide.update(cx, |pos, _cx| {
                                        *pos = -1.0;
                                    });
                                    transitions.fade_first.reset(cx);
                                    transitions.fade_first.update(cx, |pos, _cx| {
                                        *pos = 1.0;
                                    });
                                    transitions.fade_last.reset(cx);
                                    transitions.fade_last.update(cx, |pos, _cx| {
                                        *pos = 1.0;
                                    });
                                });
                            }
                            cx.notify();
                        }))
                        .on_action(cx.listener(|this, _: &NavigateDown, _window, cx| {
                            // println!("NavigateRight action received");
                            if this.options.axis == Axis::Horizontal {
                                return;
                            }
                            // println!("using axis: {:?}", this.options.axis);
                            if this.focused_index < this.num_entries.saturating_sub(1) {
                                let new_index = this.focused_index + 1;
                                this.focused_index = new_index;
                                // this.delegate.set_focused_index(Some(new_index));
                                this.transitions.as_mut().map(|transitions| {
                                    transitions.slide.reset(cx);
                                    transitions.slide.update(cx, |pos, _cx| {
                                        *pos = 1.0;
                                    });
                                    transitions.fade_first.reset(cx);
                                    transitions.fade_first.update(cx, |pos, _cx| {
                                        *pos = 1.0;
                                    });
                                    transitions.fade_last.reset(cx);
                                    transitions.fade_last.update(cx, |pos, _cx| {
                                        *pos = 1.0;
                                    });
                                });

                                cx.notify();
                            }
                        }))
                        .into_any_element(),
                    )
                }
            })
    }
}

impl<D> Focusable for GalleryState<D>
where
    D: GalleryDelegate,
{
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<D> Render for GalleryState<D>
where
    D: GalleryDelegate,
{
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare_items_if_needed(window, cx);
        // let gallery = self.render_items(window, cx);
        let focus_handle = self.focus_handle.clone();
        div()
            .key_context("Gallery")
            .id("gallery-state")
            .track_focus(&self.focus_handle)
            .size_full()
            .relative()
            .overflow_hidden()
            .child(self.render_items(window, cx).track_focus(&focus_handle))
    }
}

#[derive(IntoElement)]
pub struct Gallery<D: GalleryDelegate + 'static> {
    state: Entity<GalleryState<D>>,
    // options: GalleryOptions,
}

impl<D> Gallery<D>
where
    D: GalleryDelegate + 'static,
{
    /// Create a new Gallery element with the given GalleryState entity.
    pub fn new(state: Entity<GalleryState<D>>) -> Self {
        Self { state }
    }

    // /// Set the animation mode for the gallery
    // pub fn with_animation_mode(mut self, mode: GalleryAnimationMode) -> Self {
    //     self.options.animation_mode = mode;
    //     self
    // }

    // /// Set the animation duration for the gallery
    // pub fn with_transition_duration(mut self, duration: Duration) -> Self {
    //     self.options.transition_duration = duration;
    //     self
    // }
}

impl<D> RenderOnce for Gallery<D>
where
    D: GalleryDelegate + 'static,
{
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // self.state.update(cx, |state, _| {
        //     state.options = self.options;
        // });

        self.state.clone()
    }
}
