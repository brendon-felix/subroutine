use super::PipelineContext;
use crate::{Action, ActionState, AnyItem, Event, Routine};
use chrono::{DateTime, Duration, Local, Utc};

/// The Queue is an automatic priority queue that calculates the priority of each item based on its state
pub fn build_queue(pl: &PipelineContext) -> Vec<AnyItem> {
    let mut low_priority = Vec::new();
    let mut high_priority = Vec::new();
    let today = Local::now().date_naive();
    for event in pl.events {}
    for action in pl.actions {}
    for routine in pl.routines {}
    high_priority.extend(low_priority);
    high_priority
}
