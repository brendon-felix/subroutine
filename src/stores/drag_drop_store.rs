use gpui::Context;

use crate::stores::task_store::TaskLocation;

#[derive(Default, Debug, Clone)]
pub struct DragDropStore {
    source: Option<TaskLocation>,
    current_target: Option<TaskLocation>,
}

impl DragDropStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            source: None,
            current_target: None,
        }
    }

    pub fn set_source(&mut self, source: Option<TaskLocation>, cx: &mut Context<Self>) {
        if self.source != source {
            self.source = source;
            cx.notify();
        }
    }

    pub fn get_source(&self) -> Option<&TaskLocation> {
        self.source.as_ref()
    }

    pub fn set_target(&mut self, target: Option<TaskLocation>, cx: &mut Context<Self>) {
        if self.current_target != target {
            self.current_target = target;
            cx.notify();
        }
    }

    pub fn get_target(&self) -> Option<&TaskLocation> {
        self.current_target.as_ref()
    }

    pub fn clear_target(&mut self, cx: &mut Context<Self>) {
        self.current_target = None;
        cx.notify();
    }
}

pub trait DragDropArea {
    fn calculate_drop_index(&self, y: f32) -> usize;
}
