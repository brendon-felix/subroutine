use std::time::{Duration, Instant};

use ratatui::{layout::Rect, style::Color};
use tachyonfx::{EffectManager, EffectTimer, Interpolation, Motion, RefRect, fx};

#[derive(Debug, Clone)]
pub enum AnimationDirection {
    Left,
    Right,
    Up,
    Down,
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum AnimationType {
    ResizeTo {
        dir: AnimationDirection,
        amount: i32,
    },
    ResizeFrom {
        dir: AnimationDirection,
        amount: i32,
    },
    TranslateTo {
        x: i32,
        y: i32,
    },
    TranslateFrom {
        x: i32,
        y: i32,
    },
    Composite(Vec<AnimationType>),
    Delay(Duration, Box<AnimationType>),
}

#[derive(Debug, Clone)]
pub struct Animation {
    pub anim_type: AnimationType,
    start_instant: Instant,
    duration: Duration,
}
impl Animation {
    pub fn new(anim_type: AnimationType, duration: Duration) -> Self {
        Self {
            anim_type,
            start_instant: Instant::now(),
            duration,
        }
    }
}

// pub trait AnimatableWidget {
//     fn start_animation(&mut self, duration: Duration);
//     fn update_animation(&mut self, now: Instant);
//     fn is_animating(&self) -> bool;
// }

#[derive(Debug)]
pub struct AnimatedArea {
    area: RefRect,
    initial_area: Rect,
    starting_area: Rect,
    animation: Option<Animation>,
}

impl AnimatedArea {
    pub fn new(area: Rect) -> Self {
        Self {
            area: RefRect::new(area),
            initial_area: area,
            starting_area: area,
            animation: None,
        }
    }

    pub fn start_animation(&mut self, animation: Animation) {
        // self.initial_area = self.area;
        self.starting_area = self.area.get();
        self.animation = Some(animation);
    }

    pub fn is_completed(&self) -> bool {
        self.animation.is_none()
    }

    pub fn reset_to_initial(&mut self) {
        self.area.set(self.initial_area);
        self.animation = None;
    }

    pub fn reset_to_start(&mut self) {
        self.area.set(self.starting_area);
        self.animation = None;
    }

    pub fn update(&mut self) {
        let mut complete = false;
        if let Some(animation) = &self.animation {
            let now = Instant::now();
            let elapsed = now.duration_since(animation.start_instant);

            match &animation.anim_type {
                AnimationType::Delay(delay_duration, inner_animation) => {
                    if elapsed >= *delay_duration {
                        // Delay period is over, start the inner animation
                        let inner_elapsed = elapsed - *delay_duration;
                        if inner_elapsed >= animation.duration {
                            complete = true;
                        } else {
                            let progress =
                                inner_elapsed.as_secs_f32() / animation.duration.as_secs_f32();
                            let mut area = self.area.get();
                            self.apply_animation_type(inner_animation, progress, &mut area);
                            self.area.set(area);
                        }
                    }
                    // If we're still in the delay period, do nothing (area remains unchanged)
                }
                _ => {
                    if elapsed >= animation.duration {
                        complete = true;
                    } else {
                        let progress = elapsed.as_secs_f32() / animation.duration.as_secs_f32();
                        let mut area = self.area.get();
                        self.apply_animation_type(&animation.anim_type, progress, &mut area);
                        self.area.set(area);
                    }
                }
            }
        }
        if complete {
            self.animation = None;
            self.reset_to_initial();
        }
    }

    fn apply_animation_type(&self, anim_type: &AnimationType, progress: f32, area: &mut Rect) {
        match anim_type {
            AnimationType::ResizeTo { dir, amount } => {
                let total_change = (*amount as f32 * progress).round() as i32;
                match dir {
                    AnimationDirection::Left => {
                        let x_before = area.x;
                        area.x = (self.initial_area.x as i32).saturating_sub(total_change) as u16;
                        area.width = (self.initial_area.width as i32 + (area.x - x_before) as i32)
                            .max(0) as u16;
                    }
                    AnimationDirection::Right => {
                        area.width = (self.initial_area.width as i32 + total_change) as u16;
                    }
                    AnimationDirection::Up => {
                        area.y = (self.initial_area.y as i32 - total_change) as u16;
                        area.height =
                            (self.initial_area.height as i32 + total_change).max(0) as u16;
                    }
                    AnimationDirection::Down => {
                        area.height = (self.initial_area.height as i32 + total_change) as u16;
                    }
                    AnimationDirection::Horizontal => {
                        area.x = (self.initial_area.x as i32 - total_change) as u16;
                        area.width = (self.initial_area.width as i32 + total_change * 2) as u16;
                    }
                    AnimationDirection::Vertical => {
                        area.y = (self.initial_area.y as i32 - total_change) as u16;
                        area.height = (self.initial_area.height as i32 + total_change * 2) as u16;
                    }
                }
            }
            AnimationType::ResizeFrom { dir, amount } => {
                let total_change = (*amount as f32 * (1.0 - progress)).round() as i32;
                match dir {
                    AnimationDirection::Left => {
                        let x_before = area.x;
                        area.x = (self.initial_area.x as i32).saturating_sub(total_change) as u16;
                        area.width = (self.initial_area.width as i32 + (area.x - x_before) as i32)
                            .max(0) as u16;
                    }
                    AnimationDirection::Right => {
                        area.width = (self.initial_area.width as i32 + total_change) as u16;
                    }
                    AnimationDirection::Up => {
                        area.y = (self.initial_area.y as i32 - total_change) as u16;
                        area.height =
                            (self.initial_area.height as i32 + total_change).max(0) as u16;
                    }
                    AnimationDirection::Down => {
                        area.height = (self.initial_area.height as i32 + total_change) as u16;
                    }
                    AnimationDirection::Horizontal => {
                        area.x = (self.initial_area.x as i32 - total_change) as u16;
                        area.width = (self.initial_area.width as i32 + total_change * 2) as u16;
                    }
                    AnimationDirection::Vertical => {
                        area.y = (self.initial_area.y as i32 - total_change) as u16;
                        area.height = (self.initial_area.height as i32 + total_change * 2) as u16;
                    }
                }
            }
            AnimationType::TranslateTo { x, y } => {
                let total_change_x = (*x as f32 * progress).round() as i32;
                let total_change_y = (*y as f32 * progress).round() as i32;
                area.x = (self.initial_area.x as i32 + total_change_x) as u16;
                area.y = (self.initial_area.y as i32 + total_change_y) as u16;
            }
            AnimationType::TranslateFrom { x, y } => {
                let total_change_x = (*x as f32 * (1.0 - progress)).round() as i32;
                let total_change_y = (*y as f32 * (1.0 - progress)).round() as i32;
                area.x = (self.initial_area.x as i32 + total_change_x) as u16;
                area.y = (self.initial_area.y as i32 + total_change_y) as u16;
            }
            AnimationType::Composite(types) => {
                for anim_type in types {
                    self.apply_animation_type(anim_type, progress, area);
                }
            }
            AnimationType::Delay(_, inner_animation) => {
                // This should not be called directly, but handle it just in case
                self.apply_animation_type(inner_animation, progress, area);
            }
        }
    }

    pub fn get_area(&self) -> Rect {
        self.area.get()
    }

    pub fn get_area_ref(&self) -> &RefRect {
        &self.area
    }

    pub fn current_animation(&self) -> Option<&Animation> {
        self.animation.as_ref()
    }
}

pub fn start_sweep(n_ms: u32, effects: &mut EffectManager<()>) {
    let c = Color::Rgb(25, 25, 25);
    let timer = EffectTimer::from_ms(n_ms, Interpolation::Linear);
    let fx = fx::sweep_in(Motion::UpToDown, 10, 0, c, timer);
    effects.add_effect(fx);
}
