use std::collections::HashSet;

use uuid::Uuid;

use crate::{Context, MAX_SPOONS, PipelineEntry, check_constraints};

/// A single factor that contributes to the score of a pipeline entry.
pub struct ScoringFactor {
    pub name: &'static str,
    pub weight: f32,
    pub compute: fn(&PipelineEntry, &Context) -> f32,
}

/// The result of scoring a pipeline entry. Holds the final weighted total and a
/// per-factor breakdown for debugging and future explainability features.
#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    /// Final weighted score in 0.0–1.0. Zero means the entry failed constraint
    /// checking or scored at the minimum on all factors.
    pub total: f32,
    /// Each factor's name and its weighted contribution to the total.
    pub factors: Vec<(&'static str, f32)>,
}

impl ScoreBreakdown {
    fn zero() -> Self {
        Self {
            total: 0.0,
            factors: vec![],
        }
    }
}

static DEFAULT_FACTORS: &[ScoringFactor] = &[
    ScoringFactor {
        name: "importance",
        weight: 0.4,
        compute: importance_factor,
    },
    ScoringFactor {
        name: "attention_fit",
        weight: 0.3,
        compute: attention_fit_factor,
    },
    ScoringFactor {
        name: "deadline_urgency",
        weight: 0.2,
        compute: deadline_urgency_factor,
    },
    ScoringFactor {
        name: "energy_fit",
        weight: 0.1,
        compute: energy_fit_factor,
    },
];

/// Scores a pipeline entry against the current context. Runs constraint checking
/// as a hard gate — entries that fail constraints score zero. Otherwise, each
/// factor contributes a weighted value in 0.0–1.0 to the total.
pub fn score(
    entry: &PipelineEntry,
    context: &Context,
    completed_ids: &HashSet<Uuid>,
) -> ScoreBreakdown {
    if let Some(actionable) = entry.as_actionable() {
        if !check_constraints(actionable, context, completed_ids) {
            return ScoreBreakdown::zero();
        }
    }

    let mut total = 0.0f32;
    let mut factors = Vec::with_capacity(DEFAULT_FACTORS.len());

    for factor in DEFAULT_FACTORS {
        let raw = (factor.compute)(entry, context).clamp(0.0, 1.0);
        let weighted = raw * factor.weight;
        total += weighted;
        factors.push((factor.name, weighted));
    }

    ScoreBreakdown { total, factors }
}

/// How important the action is, independent of urgency.
/// Returns 0.5 (neutral) for non-Action entries or when importance is unset.
fn importance_factor(entry: &PipelineEntry, _context: &Context) -> f32 {
    let importance = match entry {
        PipelineEntry::Action(action) => action.context.importance,
        _ => None,
    };
    importance.map(|v| (v as f32 - 1.0) / 4.0).unwrap_or(0.5)
}

/// How well the action's required attention level matches the user's current focus mode.
///
/// Maps attention_mode (−2..+2) to a preferred attention level (1..5):
///   preferred = attention_mode + 3
/// Then scores the closeness between preferred and actual attention_level.
/// Returns 0.5 (neutral) for non-Action entries or when attention_level is unset.
fn attention_fit_factor(entry: &PipelineEntry, context: &Context) -> f32 {
    let attention_level = match entry {
        PipelineEntry::Action(action) => match action.context.attention_level {
            Some(v) => v as f32,
            None => return 0.5,
        },
        _ => return 0.5,
    };

    let preferred = context.mental_state.attention_mode() as f32 + 3.0;
    1.0 - (preferred - attention_level).abs() / 4.0
}

/// How urgent the entry is based on its deadline proximity.
///
/// Returns 0.0 if no deadline is set — deadline urgency acts as a bonus for
/// time-sensitive entries rather than penalizing entries without deadlines.
/// Scales from 0.0 (30+ days away) to 1.0 (at or past the deadline).
/// Events use their scheduled time as the deadline.
/// Routines and Subroutines use the deadline from their materialized constraints.
fn deadline_urgency_factor(entry: &PipelineEntry, context: &Context) -> f32 {
    let deadline = match entry {
        PipelineEntry::Action(action) => action.constraints.deadline,
        PipelineEntry::Event(event) => Some(event.time),
        PipelineEntry::Routine(routine) => {
            routine
                .constraints
                .materialize(context.current_time)
                .deadline
        }
        PipelineEntry::Subroutine(subroutine) => {
            subroutine
                .constraints
                .materialize(context.current_time)
                .deadline
        }
        PipelineEntry::Transition(_) => return 0.0,
    };

    let deadline = match deadline {
        Some(d) => d,
        None => return 0.0,
    };

    let days_until = (deadline - context.current_time).num_seconds() as f32 / 86400.0;
    (1.0 - days_until / 30.0).clamp(0.0, 1.0)
}

/// How well the action's energy cost fits the user's current spoon level.
///
/// When spoons are full, energy_rate has no effect (neutral 0.5).
/// As spoons deplete, energizing actions (positive energy_rate) are preferred
/// and draining actions (negative energy_rate) are penalized.
/// Returns 0.5 (neutral) for non-Action entries or when energy_rate is unset.
fn energy_fit_factor(entry: &PipelineEntry, context: &Context) -> f32 {
    let energy_rate = match entry {
        PipelineEntry::Action(action) => match action.context.energy_rate {
            Some(v) => v as f32,
            None => return 0.5,
        },
        _ => return 0.5,
    };

    let spoon_ratio =
        (context.mental_state.remaining_spoons as f32 / MAX_SPOONS as f32).clamp(0.0, 1.0);

    // energy_rate is −2..+2, normalize to −1.0..+1.0
    let normalized = energy_rate / 2.0;

    // Full spoons → neutral (0.5) regardless of energy_rate.
    // Empty spoons → energizing scores 1.0, draining scores 0.0.
    (0.5 + normalized * (1.0 - spoon_ratio)).clamp(0.0, 1.0)
}
