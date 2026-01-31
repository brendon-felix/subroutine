use gpui::Context;

use database::Action;
// use ticks::tasks::TaskID;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionLocation {
    ActionList,
    Pipeline(usize),
}

#[derive(Clone, Debug)]
pub struct ActiveDrag {
    pub action_id: String,
    pub drop_target: Option<ActionLocation>,
}

impl ActiveDrag {
    pub fn new(action_id: String) -> Self {
        Self {
            action_id,
            drop_target: None,
        }
    }

    pub fn set_drop_target(&mut self, target: Option<ActionLocation>) {
        self.drop_target = target;
    }
}

#[derive(Default, Clone)]
pub struct DragDropStore {
    active_drag: Option<ActiveDrag>,
}

impl DragDropStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { active_drag: None }
    }

    pub fn is_dragging(&self) -> bool {
        self.active_drag.is_some()
    }

    pub fn new_drag(&mut self, action_id: String, cx: &mut Context<Self>) {
        self.active_drag = Some(ActiveDrag::new(action_id));
        cx.notify();
    }

    pub fn clear_drag(&mut self, cx: &mut Context<Self>) {
        self.active_drag = None;
        cx.notify();
    }

    pub fn set_drop_target(&mut self, target: Option<ActionLocation>, cx: &mut Context<Self>) {
        if let Some(active_drag) = &mut self.active_drag {
            active_drag.set_drop_target(target);
            cx.notify();
        }
    }

    pub fn get_drop_target(&self) -> Option<&ActionLocation> {
        self.active_drag
            .as_ref()
            .and_then(|drag| drag.drop_target.as_ref())
    }

    pub fn clear_drop_target(&mut self, cx: &mut Context<Self>) {
        if let Some(active_drag) = &mut self.active_drag {
            active_drag.set_drop_target(None);
            cx.notify();
        }
    }

    pub fn get_active_drag_item(&self) -> Option<&str> {
        self.active_drag
            .as_ref()
            .map(|drag| drag.action_id.as_ref())
    }
}

pub trait DragDropArea {
    fn calculate_drop_index(&self, y: f32) -> usize;
}
