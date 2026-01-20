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
pub use item::*;
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
    pipeline,
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
    let context: Option<&str> = Some("Pipeline");
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

#[derive(Clone, Copy)]
pub struct PipelineOptions {
    pub direction: PipelineDirection,
    pub max_item_size: Size<Pixels>,
    pub animation_duration: Duration,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            direction: PipelineDirection::Right,
            max_item_size: size(px(200.0), px(300.0)),
            animation_duration: Duration::from_millis(500),
        }
    }
}

pub struct PipelineState<D: PipelineDelegate> {
    pub focus_handle: FocusHandle,
    options: PipelineOptions,
    delegate: D,
    num_entries: usize,
    focused_index: usize,
}

impl<D> PipelineState<D>
where
    D: PipelineDelegate,
{
    pub fn new(delegate: D, options: PipelineOptions, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            options,
            delegate,
            focused_index: 0,
            num_entries: 0,
            // transitions: None,
        }
    }

    pub fn delegate_mut(&mut self) -> &mut D {
        &mut self.delegate
    }

    fn prepare_items_if_needed(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.num_entries = self.delegate.items_count(cx)
    }

    pub fn set_focused_index(&mut self, index: usize) {
        self.focused_index = index;
    }

    fn render_pipeline_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + Styled {
        let is_focused = self.focused_index == ix;
        div().size_full()
        // .children(self.delegate.render_item(ix, window, cx).map(|item| {
        //     item.size_full()
        //         .flex()
        //         .items_center()
        //         .selected(is_focused)
        //         .on_click(cx.listener(move |this, _: &ClickEvent, _window, _cx| {
        //             this.focused_index = ix;
        //             // this.confirm(
        //             //     &Confirm {
        //             //         secondary: e.modifiers().secondary(),
        //             //     },
        //             //     window,
        //             //     cx,
        //             // );
        //         }))
        // }))
    }

    pub fn render_items(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + InteractiveElement {
        div()
            .size_full()
            .relative()
            .when(self.num_entries == 0, |this| {
                this.child(self.delegate.render_empty(window, cx))
            })
            .when(self.num_entries > 0, {
                |this| {
                    this.child(
                        pipeline(
                            cx.entity(),
                            "pipeline",
                            self.options.direction,
                            self.num_entries,
                            self.options.max_item_size,
                            move |this, visible_range: Range<usize>, window, cx| {
                                visible_range
                                    .map(|ix| {
                                        this.render_pipeline_item(ix, window, cx).into_any_element()
                                    })
                                    .collect::<Vec<_>>()
                            },
                        )
                        // .focused_index(self.focused_index)
                        .gap_8()
                        .track_focus(&self.focus_handle)
                        .on_action(cx.listener(|this, _: &NavigateLeft, _window, cx| {
                            if this.options.direction.is_horizontal() && this.focused_index > 0 {
                                let new_index = this.focused_index - 1;
                                this.focused_index = new_index;

                                cx.notify();
                            }
                        }))
                        .on_action(cx.listener(|this, _: &NavigateRight, _window, cx| {
                            if this.options.direction.is_horizontal()
                                && this.focused_index < this.num_entries.saturating_sub(1)
                            {
                                let new_index = this.focused_index + 1;
                                this.focused_index = new_index;
                                cx.notify();
                            }
                        }))
                        .on_action(cx.listener(|this, _: &NavigateUp, _window, cx| {
                            if this.options.direction.is_vertical() && this.focused_index > 0 {
                                let new_index = this.focused_index - 1;
                                this.focused_index = new_index;

                                cx.notify();
                            }
                        }))
                        .on_action(cx.listener(|this, _: &NavigateDown, _window, cx| {
                            if this.options.direction.is_vertical()
                                && this.focused_index < this.num_entries.saturating_sub(1)
                            {
                                let new_index = this.focused_index + 1;
                                this.focused_index = new_index;
                                cx.notify();
                            }
                        }))
                        .into_any_element(),
                    )
                }
            })
    }
}

impl<D> Focusable for PipelineState<D>
where
    D: PipelineDelegate,
{
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<D> Render for PipelineState<D>
where
    D: PipelineDelegate,
{
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare_items_if_needed(window, cx);
        // let gallery = self.render_items(window, cx);
        let focus_handle = self.focus_handle.clone();
        div()
            .key_context("pipeline")
            .id("pipeline-state")
            .track_focus(&self.focus_handle)
            .size_full()
            .relative()
            .overflow_hidden()
            .child(self.render_items(window, cx).track_focus(&focus_handle))
    }
}

#[derive(IntoElement)]
pub struct Pipeline<D: PipelineDelegate + 'static> {
    state: Entity<PipelineState<D>>,
    // options: GalleryOptions,
}

impl<D> Pipeline<D>
where
    D: PipelineDelegate + 'static,
{
    /// Create a new Gallery element with the given GalleryState entity.
    pub fn new(state: Entity<PipelineState<D>>) -> Self {
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

impl<D> RenderOnce for Pipeline<D>
where
    D: PipelineDelegate + 'static,
{
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // self.state.update(cx, |state, _| {
        //     state.options = self.options;
        // });

        self.state.clone()
    }
}
