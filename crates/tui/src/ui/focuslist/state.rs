use ratatui::layout::Rect;
use tachyonfx::RefRect;

use crate::ui::animate::{AnimatedArea, Animation};

#[derive(Clone, Copy, Debug)]
pub enum FocusListPosition {
    PrevPrev,
    Prev,
    Focused,
    Next,
    NextNext,
}

#[derive(Default)]
pub struct FocusListState {
    pub prev_prev_area: Option<AnimatedArea>,
    pub prev_area: Option<AnimatedArea>,
    pub focused_area: Option<AnimatedArea>,
    pub next_area: Option<AnimatedArea>,
    pub next_next_area: Option<AnimatedArea>,
}

impl FocusListState {
    pub fn update_animations(&mut self) {
        if let Some(animated_area) = &mut self.prev_prev_area {
            animated_area.update();
        }
        if let Some(animated_area) = &mut self.prev_area {
            animated_area.update();
        }
        if let Some(animated_area) = &mut self.focused_area {
            animated_area.update();
        }
        if let Some(animated_area) = &mut self.next_area {
            animated_area.update();
        }
        if let Some(animated_area) = &mut self.next_next_area {
            animated_area.update();
        }
    }

    pub fn set_area(&mut self, area: Rect, position: FocusListPosition) {
        match position {
            FocusListPosition::PrevPrev => {
                self.prev_prev_area = Some(AnimatedArea::new(area));
            }
            FocusListPosition::Prev => {
                self.prev_area = Some(AnimatedArea::new(area));
            }
            FocusListPosition::Focused => {
                self.focused_area = Some(AnimatedArea::new(area));
            }
            FocusListPosition::Next => {
                self.next_area = Some(AnimatedArea::new(area));
            }
            FocusListPosition::NextNext => {
                self.next_next_area = Some(AnimatedArea::new(area));
            }
        }
    }

    pub fn reset_areas(&mut self) {
        self.prev_prev_area = None;
        self.prev_area = None;
        self.focused_area = None;
        self.next_area = None;
        self.next_next_area = None;
    }

    pub fn start_animation(&mut self, animation: Animation, position: FocusListPosition) {
        match position {
            FocusListPosition::PrevPrev => {
                if let Some(animated_area) = &mut self.prev_prev_area {
                    animated_area.start_animation(animation);
                }
            }
            FocusListPosition::Prev => {
                if let Some(animated_area) = &mut self.prev_area {
                    animated_area.start_animation(animation);
                }
            }
            FocusListPosition::Focused => {
                if let Some(animated_area) = &mut self.focused_area {
                    animated_area.start_animation(animation);
                }
            }
            FocusListPosition::Next => {
                if let Some(animated_area) = &mut self.next_area {
                    animated_area.start_animation(animation);
                }
            }
            FocusListPosition::NextNext => {
                if let Some(animated_area) = &mut self.next_next_area {
                    animated_area.start_animation(animation);
                }
            }
        }
    }

    pub fn is_completed(&self, position: FocusListPosition) -> bool {
        if let Some(animated_area) = &self.get_anim_area(position) {
            animated_area.is_completed()
        } else {
            true
        }
    }

    pub fn get_anim_area(&self, position: FocusListPosition) -> Option<&AnimatedArea> {
        match position {
            FocusListPosition::PrevPrev => self.prev_prev_area.as_ref(),
            FocusListPosition::Prev => self.prev_area.as_ref(),
            FocusListPosition::Focused => self.focused_area.as_ref(),
            FocusListPosition::Next => self.next_area.as_ref(),
            FocusListPosition::NextNext => self.next_next_area.as_ref(),
        }
    }

    pub fn get_area(&self, position: FocusListPosition) -> Option<Rect> {
        let animated_area_opt = self.get_anim_area(position);
        if let Some(animated_area) = animated_area_opt {
            Some(animated_area.get_area())
        } else {
            None
        }
    }

    pub fn get_area_ref(&self, position: FocusListPosition) -> Option<RefRect> {
        let animated_area_opt = self.get_anim_area(position);
        if let Some(animated_area) = animated_area_opt {
            Some(animated_area.get_area_ref().clone())
        } else {
            None
        }
    }
}
