use chrono::{DateTime, Local, NaiveDate, Utc};

use crate::{Action, ActionState, Event, Routine};

pub struct PipelineContext<'a> {
    pub actions: &'a [Action],
    pub events: &'a [Event],
    pub routines: &'a [Routine],
    pub today: NaiveDate,
    pub now: DateTime<Utc>,
    // user_stress: StressLevel,
}

impl<'a> PipelineContext<'a> {
    pub fn new(actions: &'a [Action], events: &'a [Event], routines: &'a [Routine]) -> Self {
        let today = Local::now().date_naive();
        let now = Utc::now();
        Self {
            actions,
            events,
            routines,
            today,
            now,
        }
    }

    /// Returns the backlog of actions, sorted by date (today's backlog first, then undated backlog, then future backlog)
    pub fn backlog(&self) -> Vec<&Action> {
        let mut backlog = self
            .actions
            .iter()
            .filter_map(|a| match a.state {
                ActionState::Backlogged(date_opt) => Some((a, date_opt)),
                _ => None,
            })
            .collect::<Vec<_>>();
        // today's backlog, followed by undated backlog, followed by future backlog
        let today = self.today;
        backlog.sort_by_key(|(action, date_opt)| match date_opt {
            Some(date) if *date == today => (0, None, action.id),
            None => (1, None, action.id),
            Some(date) => (2, Some(*date), action.id),
        });
        backlog.into_iter().map(|(action, _)| action).collect()
    }

    /// Returns the unexpired events, sorted by time.
    pub fn unexpired_events(&self) -> Vec<&Event> {
        let mut unexpired = self
            .events
            .iter()
            .filter(|e| !e.is_expired(self.now))
            .collect::<Vec<_>>();
        unexpired.sort_by_key(|e| e.time);
        unexpired
    }
}

// impl PipelineContext {
//     pub fn new(actions: Vec<Action>, events: Vec<Event>, routines: Vec<Routine>) -> Self {
//         let queue = build_queue(&actions, &events, &routines);
//         Self {
//             // actions,
//             // events,
//             // routines,
//             queue,
//         }
//     }

//     pub fn rebuild_queue(
//         &mut self,
//         actions: Vec<Action>,
//         events: Vec<Event>,
//         routines: Vec<Routine>,
//     ) {
//         self.queue = build_queue(&actions, &events, &routines);
//     }

//     // pub fn actions(&self) -> &[Action] {
//     //     &self.actions
//     // }

//     // pub fn incomplete_actions(&self) -> Vec<&Action> {
//     //     self.actions
//     //         .iter()
//     //         .filter(|a| !a.is_completed())
//     //         .collect::<Vec<_>>()
//     // }

//     // pub fn backlog(&self) -> Vec<&Action> {
//     //     self.actions
//     //         .iter()
//     //         .filter(|a| a.is_backlogged())
//     //         .collect::<Vec<_>>()
//     // }

//     // pub fn events(&self) -> &[Event] {
//     //     &self.events
//     // }

//     // pub fn unexpired_events(&self) -> Vec<&Event> {
//     //     let now = chrono::Utc::now();
//     //     self.events
//     //         .iter()
//     //         .filter(|e| !e.is_expired(now))
//     //         .collect::<Vec<_>>()
//     // }

//     // pub fn routines(&self) -> &[Routine] {
//     //     &self.routines
//     // }

//     pub fn push_item(&mut self, item: AnyItem) {
//         // match item {
//         //     AnyItem::Action(action) => self.actions.push(action),
//         //     AnyItem::Event(event) => self.events.push(event),
//         //     AnyItem::Routine(routine) => self.routines.push(routine),
//         // }
//         // self.rebuild_queue();
//         s
//     }

//     pub fn pop_item(&mut self) -> Option<AnyItem> {
//         self.queue.pop()
//     }
// }
