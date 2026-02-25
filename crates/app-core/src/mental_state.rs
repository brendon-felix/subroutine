use chrono::Duration;
use uuid::Uuid;

use crate::Action;

/// The maximum spoon count, representing a fully-rested user.
pub const MAX_SPOONS: u32 = 10;

/// Spoons recovered per hour of elapsed time.
pub const SPOON_RECOVERY_RATE: f32 = 2.0;

/// Base spoon cost of completing any action, before energy_rate adjustment.
const BASE_ACTION_COST: i32 = 1;

/// A reusable mental state profile that captures a snapshot of how the user is feeling
/// across four bipolar axes. Created by the user and used to quickly declare their
/// current mental state.
///
/// All axes use the range -2 (one extreme) to +2 (the other extreme), with 0 as neutral.
#[derive(Debug, Clone)]
pub struct SavedMentalState {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    /// scattered (-2) <-> hyperfocused (+2)
    pub attention_mode: i8,
    /// understimulated (-2) <-> overstimulated (+2)
    pub sensory_tolerance: i8,
    /// dysregulated (-2) <-> regulated (+2)
    pub emotional_regulation: i8,
    /// drained (-2) <-> charged (+2)
    pub social_battery: i8,
}

impl SavedMentalState {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            attention_mode: 0,
            sensory_tolerance: 0,
            emotional_regulation: 0,
            social_battery: 0,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_axes(
        mut self,
        attention_mode: i8,
        sensory_tolerance: i8,
        emotional_regulation: i8,
        social_battery: i8,
    ) -> Self {
        self.attention_mode = attention_mode;
        self.sensory_tolerance = sensory_tolerance;
        self.emotional_regulation = emotional_regulation;
        self.social_battery = social_battery;
        self
    }
}

/// The current mental state, combining derived spoon tracking with declared axis values.
#[derive(Debug, Clone)]
pub struct MentalState {
    /// Derived from completed action costs and time-based recovery. Represents available
    /// executive function capacity.
    pub remaining_spoons: u32,
    /// The declared state the user is currently in, if any.
    pub declared: Option<SavedMentalState>,
}

impl MentalState {
    pub fn new(remaining_spoons: u32) -> Self {
        Self {
            remaining_spoons,
            declared: None,
        }
    }

    pub fn with_declared(mut self, state: SavedMentalState) -> Self {
        self.declared = Some(state);
        self
    }

    pub fn attention_mode(&self) -> i8 {
        self.declared.as_ref().map_or(0, |s| s.attention_mode)
    }

    pub fn sensory_tolerance(&self) -> i8 {
        self.declared.as_ref().map_or(0, |s| s.sensory_tolerance)
    }

    pub fn emotional_regulation(&self) -> i8 {
        self.declared.as_ref().map_or(0, |s| s.emotional_regulation)
    }

    pub fn social_battery(&self) -> i8 {
        self.declared.as_ref().map_or(0, |s| s.social_battery)
    }

    /// Updates remaining spoons to reflect the cost of completing an action.
    ///
    /// Cost = base cost (1) minus energy_rate. Draining actions cost more;
    /// energizing actions can restore spoons. Examples:
    ///   energy_rate = -2 → costs 3 spoons
    ///   energy_rate =  0 → costs 1 spoon
    ///   energy_rate = +2 → restores 1 spoon
    pub fn complete_action(&mut self, action: &Action) {
        let energy_rate = action.context.energy_rate.unwrap_or(0) as i32;
        let net_cost = BASE_ACTION_COST - energy_rate;

        if net_cost > 0 {
            self.remaining_spoons = self.remaining_spoons.saturating_sub(net_cost as u32);
        } else {
            self.remaining_spoons = (self.remaining_spoons + (-net_cost) as u32).min(MAX_SPOONS);
        }
    }

    /// Recovers spoons based on elapsed time, capped at MAX_SPOONS.
    /// Intended to be called periodically with the time since the last recovery tick.
    pub fn recover_spoons(&mut self, elapsed: Duration) {
        if elapsed.num_seconds() <= 0 {
            return;
        }
        let hours = elapsed.num_seconds() as f32 / 3600.0;
        let restored = (hours * SPOON_RECOVERY_RATE) as u32;
        self.remaining_spoons = (self.remaining_spoons + restored).min(MAX_SPOONS);
    }
}

pub mod starter_states {
    use uuid::{Uuid, uuid};

    use super::SavedMentalState;

    pub const COASTING_ID: Uuid = uuid!("a1000000-0000-0000-0000-000000000001");
    pub const ENERGIZED_ID: Uuid = uuid!("a1000000-0000-0000-0000-000000000002");
    pub const FOCUSED_ID: Uuid = uuid!("a1000000-0000-0000-0000-000000000003");
    pub const SCATTERED_ID: Uuid = uuid!("a1000000-0000-0000-0000-000000000004");
    pub const TIRED_ID: Uuid = uuid!("a1000000-0000-0000-0000-000000000005");
    pub const FOGGY_ID: Uuid = uuid!("a1000000-0000-0000-0000-000000000006");
    pub const OVERWHELMED_ID: Uuid = uuid!("a1000000-0000-0000-0000-000000000007");
    pub const FRIED_ID: Uuid = uuid!("a1000000-0000-0000-0000-000000000008");

    pub fn coasting() -> SavedMentalState {
        SavedMentalState {
            id: COASTING_ID,
            name: "Coasting".into(),
            description: Some("Balanced and neutral across all dimensions.".into()),
            attention_mode: 0,
            sensory_tolerance: 0,
            emotional_regulation: 0,
            social_battery: 0,
        }
    }

    pub fn energized() -> SavedMentalState {
        SavedMentalState {
            id: ENERGIZED_ID,
            name: "Energized".into(),
            description: Some("High energy, emotionally steady, socially open.".into()),
            attention_mode: 1,
            sensory_tolerance: 0,
            emotional_regulation: 1,
            social_battery: 1,
        }
    }

    pub fn focused() -> SavedMentalState {
        SavedMentalState {
            id: FOCUSED_ID,
            name: "Focused".into(),
            description: Some(
                "In the zone. Deep work is accessible but transitions are hard.".into(),
            ),
            attention_mode: 2,
            sensory_tolerance: 0,
            emotional_regulation: 1,
            social_battery: -1,
        }
    }

    pub fn scattered() -> SavedMentalState {
        SavedMentalState {
            id: SCATTERED_ID,
            name: "Scattered".into(),
            description: Some(
                "Attention keeps jumping. Short varied tasks work better than deep focus.".into(),
            ),
            attention_mode: -2,
            sensory_tolerance: 1,
            emotional_regulation: 0,
            social_battery: 0,
        }
    }

    pub fn tired() -> SavedMentalState {
        SavedMentalState {
            id: TIRED_ID,
            name: "Tired".into(),
            description: Some("Low energy and attention. Needs low-demand, familiar tasks.".into()),
            attention_mode: -1,
            sensory_tolerance: -1,
            emotional_regulation: 0,
            social_battery: -1,
        }
    }

    pub fn foggy() -> SavedMentalState {
        SavedMentalState {
            id: FOGGY_ID,
            name: "Foggy".into(),
            description: Some(
                "Emotionally off and low on focus. Gentle, low-stakes tasks only.".into(),
            ),
            attention_mode: -1,
            sensory_tolerance: 0,
            emotional_regulation: -1,
            social_battery: -1,
        }
    }

    pub fn overwhelmed() -> SavedMentalState {
        SavedMentalState {
            id: OVERWHELMED_ID,
            name: "Overwhelmed".into(),
            description: Some(
                "Too much input, too little capacity. Needs calm, minimal, solitary tasks.".into(),
            ),
            attention_mode: -2,
            sensory_tolerance: 2,
            emotional_regulation: -2,
            social_battery: -2,
        }
    }

    pub fn fried() -> SavedMentalState {
        SavedMentalState {
            id: FRIED_ID,
            name: "Fried".into(),
            description: Some("Fully depleted. Rest and recovery, not tasks.".into()),
            attention_mode: -1,
            sensory_tolerance: -1,
            emotional_regulation: -2,
            social_battery: -2,
        }
    }

    pub fn all() -> Vec<SavedMentalState> {
        vec![
            coasting(),
            energized(),
            focused(),
            scattered(),
            tired(),
            foggy(),
            overwhelmed(),
            fried(),
        ]
    }
}
