//! Scoring system for ranking action instances based on context.
//!
//! This module provides a composable, extensible framework for scoring instances.
//! Individual scoring factors implement the `Scorer` trait and can be combined
//! with configurable weights using the `ScoringEngine`.

use crate::{Action, Instance};
use anyhow::Result;
use chrono::{DateTime, Datelike, Timelike, Utc};
use rusqlite::Connection;
use std::collections::HashMap;

// ============================================================================
// Core Types
// ============================================================================

/// Context information used for scoring decisions.
#[derive(Debug, Clone)]
pub struct ScoringContext {
    pub current_time: DateTime<Utc>,
    pub time_of_day: TimeOfDay,
    pub day_type: DayType,

    // Current state
    pub mental_state: Option<String>,
    pub energy_level: Option<f64>,       // 0.0-1.0
    pub attention_capacity: Option<f64>, // 0.0-1.0

    // Environment
    pub environment: Vec<String>,
    pub location: Vec<String>,

    // User preferences (learned over time)
    pub user_preferences: HashMap<String, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDay {
    Morning,   // 5am-12pm
    Afternoon, // 12pm-5pm
    Evening,   // 5pm-9pm
    Night,     // 9pm-5am
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayType {
    Weekday,
    Weekend,
}

impl ScoringContext {
    /// Create a basic scoring context with current time.
    pub fn basic() -> Self {
        let current_time = Utc::now();
        let time_of_day = TimeOfDay::from_hour(current_time.hour());
        let day_type = DayType::from_weekday(current_time.weekday());

        Self {
            current_time,
            time_of_day,
            day_type,
            mental_state: None,
            energy_level: None,
            attention_capacity: None,
            environment: Vec::new(),
            location: Vec::new(),
            user_preferences: HashMap::new(),
        }
    }

    /// Set the mental state.
    pub fn with_mental_state(mut self, state: String) -> Self {
        self.mental_state = Some(state);
        self
    }

    /// Set the energy level (0.0-1.0).
    pub fn with_energy_level(mut self, level: f64) -> Self {
        self.energy_level = Some(level.clamp(0.0, 1.0));
        self
    }

    /// Set the attention capacity (0.0-1.0).
    pub fn with_attention_capacity(mut self, capacity: f64) -> Self {
        self.attention_capacity = Some(capacity.clamp(0.0, 1.0));
        self
    }

    /// Add an environment tag.
    pub fn with_environment(mut self, env: String) -> Self {
        self.environment.push(env);
        self
    }

    /// Add a location tag.
    pub fn with_location(mut self, location: String) -> Self {
        self.location.push(location);
        self
    }
}

impl TimeOfDay {
    pub fn from_hour(hour: u32) -> Self {
        match hour {
            5..=11 => TimeOfDay::Morning,
            12..=16 => TimeOfDay::Afternoon,
            17..=20 => TimeOfDay::Evening,
            _ => TimeOfDay::Night,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            TimeOfDay::Morning => "morning",
            TimeOfDay::Afternoon => "afternoon",
            TimeOfDay::Evening => "evening",
            TimeOfDay::Night => "night",
        }
    }
}

impl DayType {
    pub fn from_weekday(weekday: chrono::Weekday) -> Self {
        use chrono::Weekday;
        match weekday {
            Weekday::Sat | Weekday::Sun => DayType::Weekend,
            _ => DayType::Weekday,
        }
    }
}

// ============================================================================
// Scorer Trait
// ============================================================================

/// Trait for individual scoring factors.
///
/// Each scorer evaluates one aspect of how well an instance fits the current context.
/// Scores typically range from 0.0 (poor match) to 1.0 (perfect match), but can
/// exceed these bounds for exceptional or very poor matches.
pub trait Scorer: Send + Sync {
    /// Calculate a score for an instance in the given context.
    ///
    /// Returns:
    /// - 1.0 for perfect match
    /// - 0.5 for neutral/moderate match
    /// - 0.0 for poor match
    /// - Can exceed 1.0 for exceptional matches
    /// - Can be negative for strong mismatches
    fn score(&self, instance: &Instance, action: &Action, context: &ScoringContext) -> Result<f64>;

    /// Human-readable name for this scoring factor.
    fn name(&self) -> &str;

    /// Optional explanation of why this score was given.
    fn explain(&self, _score: f64, _action: &Action, _context: &ScoringContext) -> Option<String> {
        None
    }
}

// ============================================================================
// Scored Results
// ============================================================================

/// A scored instance with explanation of contributing factors.
#[derive(Debug, Clone)]
pub struct ScoredInstance {
    pub instance_id: String,
    pub action_id: String,
    pub total_score: f64,
    pub factor_scores: Vec<FactorScore>,
}

/// Contribution of a single scoring factor.
#[derive(Debug, Clone)]
pub struct FactorScore {
    pub factor_name: String,
    pub raw_score: f64,
    pub weight: f64,
    pub weighted_score: f64,
    pub explanation: Option<String>,
}

impl ScoredInstance {
    /// Generate a human-readable explanation of the score.
    pub fn explain(&self) -> String {
        let mut explanation = format!("Total score: {:.2}\n\nBreakdown:", self.total_score);

        for factor in &self.factor_scores {
            explanation.push_str(&format!(
                "\n  {}: {:.2} × {:.1} = {:.2}",
                factor.factor_name, factor.raw_score, factor.weight, factor.weighted_score
            ));
            if let Some(ref explain) = factor.explanation {
                explanation.push_str(&format!("\n    {}", explain));
            }
        }

        explanation
    }
}

// ============================================================================
// Scoring Engine
// ============================================================================

/// A configured scoring factor with its weight.
pub struct ScoringFactor {
    pub name: String,
    pub weight: f64,
    pub scorer: Box<dyn Scorer>,
    pub enabled: bool,
}

/// Combines multiple scoring factors to produce a final score.
pub struct ScoringEngine {
    factors: Vec<ScoringFactor>,
}

impl ScoringEngine {
    /// Create a new empty scoring engine.
    pub fn new() -> Self {
        Self {
            factors: Vec::new(),
        }
    }

    /// Add a scoring factor with a given weight.
    pub fn add_factor(&mut self, name: String, weight: f64, scorer: Box<dyn Scorer>) {
        self.factors.push(ScoringFactor {
            name,
            weight,
            scorer,
            enabled: true,
        });
    }

    /// Enable or disable a specific factor by name.
    pub fn set_factor_enabled(&mut self, name: &str, enabled: bool) {
        if let Some(factor) = self.factors.iter_mut().find(|f| f.name == name) {
            factor.enabled = enabled;
        }
    }

    /// Update the weight of a specific factor.
    pub fn set_factor_weight(&mut self, name: &str, weight: f64) {
        if let Some(factor) = self.factors.iter_mut().find(|f| f.name == name) {
            factor.weight = weight;
        }
    }

    /// Score a single instance.
    pub fn score_instance(
        &self,
        instance: &Instance,
        action: &Action,
        context: &ScoringContext,
    ) -> Result<ScoredInstance> {
        let mut factor_scores = Vec::new();
        let mut total_score = 0.0;

        for factor in &self.factors {
            if !factor.enabled {
                continue;
            }

            let raw_score = factor.scorer.score(instance, action, context)?;
            let weighted_score = raw_score * factor.weight;
            let explanation = factor.scorer.explain(raw_score, action, context);

            factor_scores.push(FactorScore {
                factor_name: factor.name.clone(),
                raw_score,
                weight: factor.weight,
                weighted_score,
                explanation,
            });

            total_score += weighted_score;
        }

        // Normalize by total weight of enabled factors
        let total_weight: f64 = self
            .factors
            .iter()
            .filter(|f| f.enabled)
            .map(|f| f.weight)
            .sum();

        if total_weight > 0.0 {
            total_score /= total_weight;
        }

        Ok(ScoredInstance {
            instance_id: instance.id.clone(),
            action_id: instance.action_id.clone(),
            total_score,
            factor_scores,
        })
    }

    /// Score multiple instances and return them sorted by score (descending).
    pub fn score_instances(
        &self,
        items: Vec<(Instance, Action)>,
        context: &ScoringContext,
    ) -> Result<Vec<ScoredInstance>> {
        let mut scored: Vec<ScoredInstance> = items
            .into_iter()
            .map(|(instance, action)| self.score_instance(&instance, &action, context))
            .collect::<Result<Vec<_>>>()?;

        scored.sort_by(|a, b| {
            b.total_score
                .partial_cmp(&a.total_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(scored)
    }
}

impl Default for ScoringEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Phase 1: Basic Scorers
// ============================================================================

/// Matches action's preferred time of day against current time.
pub struct TimeOfDayScorer;

impl Scorer for TimeOfDayScorer {
    fn score(
        &self,
        _instance: &Instance,
        action: &Action,
        context: &ScoringContext,
    ) -> Result<f64> {
        let preferred_times = action.preferred_time_of_day_vec();

        if preferred_times.is_empty() {
            return Ok(0.5); // Neutral if no preference
        }

        let current = context.time_of_day.as_str();

        // Check for exact match
        if preferred_times.iter().any(|tod| tod == current) {
            return Ok(1.0);
        }

        // Check for adjacent time (e.g., morning -> afternoon)
        let adjacent_score = match context.time_of_day {
            TimeOfDay::Morning => {
                if preferred_times.iter().any(|tod| tod == "afternoon") {
                    0.5
                } else {
                    0.0
                }
            }
            TimeOfDay::Afternoon => {
                if preferred_times
                    .iter()
                    .any(|tod| tod == "morning" || tod == "evening")
                {
                    0.5
                } else {
                    0.0
                }
            }
            TimeOfDay::Evening => {
                if preferred_times
                    .iter()
                    .any(|tod| tod == "afternoon" || tod == "night")
                {
                    0.5
                } else {
                    0.0
                }
            }
            TimeOfDay::Night => {
                if preferred_times.iter().any(|tod| tod == "evening") {
                    0.5
                } else {
                    0.0
                }
            }
        };

        Ok(adjacent_score)
    }

    fn name(&self) -> &str {
        "time_of_day"
    }

    fn explain(&self, score: f64, action: &Action, context: &ScoringContext) -> Option<String> {
        let preferred_times = action.preferred_time_of_day_vec();

        if preferred_times.is_empty() {
            return Some("No time preference set".to_string());
        }

        Some(format!(
            "Current time is {}, preferred times: {}. Score: {:.2}",
            context.time_of_day.as_str(),
            preferred_times.join(", "),
            score
        ))
    }
}

/// Considers action duration against available time.
pub struct DurationScorer {
    pub available_minutes: Option<i64>,
}

impl DurationScorer {
    pub fn new() -> Self {
        Self {
            available_minutes: None,
        }
    }

    pub fn with_available_time(available_minutes: i64) -> Self {
        Self {
            available_minutes: Some(available_minutes),
        }
    }
}

impl Default for DurationScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl Scorer for DurationScorer {
    fn score(
        &self,
        _instance: &Instance,
        action: &Action,
        _context: &ScoringContext,
    ) -> Result<f64> {
        let action_duration = action.duration_bucket.unwrap_or(0);

        if action_duration == 0 {
            return Ok(0.5); // Neutral if no duration set
        }

        // Fibonacci minutes: 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144
        let duration_minutes = match action_duration {
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 5,
            5 => 8,
            6 => 13,
            7 => 21,
            8 => 34,
            9 => 55,
            10 => 89,
            11 => 144,
            _ => return Ok(0.5),
        };

        if let Some(available) = self.available_minutes {
            if duration_minutes > available {
                // Too long for available time
                Ok(0.0)
            } else if duration_minutes <= available / 2 {
                // Comfortably fits
                Ok(1.0)
            } else {
                // Fits but tight - linear scale
                Ok((available - duration_minutes) as f64 / (available / 2) as f64)
            }
        } else {
            // No time constraint - prefer shorter tasks slightly
            if duration_minutes <= 13 {
                Ok(0.8)
            } else if duration_minutes <= 34 {
                Ok(0.6)
            } else {
                Ok(0.4)
            }
        }
    }

    fn name(&self) -> &str {
        "duration"
    }

    fn explain(&self, score: f64, action: &Action, _context: &ScoringContext) -> Option<String> {
        let duration = action.duration_bucket.unwrap_or(0);
        if duration == 0 {
            return Some("No duration set".to_string());
        }

        if let Some(available) = self.available_minutes {
            Some(format!(
                "Task duration: ~{} min, Available: {} min. Score: {:.2}",
                duration, available, score
            ))
        } else {
            Some(format!(
                "Task duration bucket: {}. Score: {:.2}",
                duration, score
            ))
        }
    }
}

/// Matches action's energy rate against current energy level.
pub struct EnergyScorer;

impl Scorer for EnergyScorer {
    fn score(
        &self,
        _instance: &Instance,
        action: &Action,
        context: &ScoringContext,
    ) -> Result<f64> {
        let Some(current_energy) = context.energy_level else {
            return Ok(0.5); // Neutral if no energy level known
        };

        let energy_rate = action.energy_rate.unwrap_or(0) as f64;

        if energy_rate == 0.0 {
            return Ok(0.5); // Neutral if no energy rate set
        }

        // Normalize energy_rate to 0.0-1.0 scale (assuming 1-5 scale)
        let normalized_rate = energy_rate / 5.0;

        // Score is high when energy level exceeds requirement
        if current_energy >= normalized_rate {
            Ok(1.0)
        } else {
            // Linearly decrease score as energy becomes insufficient
            Ok(current_energy / normalized_rate)
        }
    }

    fn name(&self) -> &str {
        "energy"
    }

    fn explain(&self, score: f64, action: &Action, context: &ScoringContext) -> Option<String> {
        if let Some(energy) = context.energy_level {
            Some(format!(
                "Your energy: {:.0}%, Task energy rate: {}/5. Score: {:.2}",
                energy * 100.0,
                action.energy_rate.unwrap_or(0),
                score
            ))
        } else {
            Some("Energy level unknown".to_string())
        }
    }
}

/// Matches action's attention level against current attention capacity.
pub struct AttentionScorer;

impl Scorer for AttentionScorer {
    fn score(
        &self,
        _instance: &Instance,
        action: &Action,
        context: &ScoringContext,
    ) -> Result<f64> {
        let Some(current_attention) = context.attention_capacity else {
            return Ok(0.5); // Neutral if no attention capacity known
        };

        let attention_level = action.attention_level.unwrap_or(0) as f64;

        if attention_level == 0.0 {
            return Ok(0.5); // Neutral if no attention level set
        }

        // Normalize attention_level to 0.0-1.0 scale (assuming 1-5 scale)
        let normalized_level = attention_level / 5.0;

        // Score is high when attention capacity exceeds requirement
        if current_attention >= normalized_level {
            Ok(1.0)
        } else if current_attention >= normalized_level * 0.7 {
            // Can probably manage
            Ok(0.7)
        } else {
            // Insufficient attention
            Ok(0.2)
        }
    }

    fn name(&self) -> &str {
        "attention"
    }

    fn explain(&self, score: f64, action: &Action, context: &ScoringContext) -> Option<String> {
        if let Some(attention) = context.attention_capacity {
            Some(format!(
                "Your attention: {:.0}%, Task requires: {}/5. Score: {:.2}",
                attention * 100.0,
                action.attention_level.unwrap_or(0),
                score
            ))
        } else {
            Some("Attention capacity unknown".to_string())
        }
    }
}

/// Considers action importance and urgency growth.
pub struct UrgencyScorer;

impl Scorer for UrgencyScorer {
    fn score(&self, instance: &Instance, action: &Action, context: &ScoringContext) -> Result<f64> {
        let importance = action.importance.unwrap_or(0) as f64;

        if importance == 0.0 {
            return Ok(0.3); // Low score for unimportant tasks
        }

        // Base score from importance (1-5 scale)
        let mut score = importance / 5.0;

        // Boost score if urgency grows over time
        if action.urgency_growth.unwrap_or(false) {
            let created_at = instance.created_at_datetime();
            let age_hours = context
                .current_time
                .signed_duration_since(created_at)
                .num_hours();

            // Add up to 0.3 based on age (maxing at 7 days)
            let age_boost = (age_hours as f64 / (7.0 * 24.0)).min(0.3);
            score += age_boost;
        }

        Ok(score.min(1.5)) // Cap at 1.5 for very urgent items
    }

    fn name(&self) -> &str {
        "urgency"
    }

    fn explain(&self, score: f64, action: &Action, _context: &ScoringContext) -> Option<String> {
        let importance = action.importance.unwrap_or(0);
        if action.urgency_growth.unwrap_or(false) {
            Some(format!(
                "Importance: {}/5, grows more urgent over time. Score: {:.2}",
                importance, score
            ))
        } else {
            Some(format!("Importance: {}/5. Score: {:.2}", importance, score))
        }
    }
}

// ============================================================================
// Default Engine Builder
// ============================================================================

/// Create a scoring engine with default Phase 1 scorers and weights.
pub fn default_scoring_engine() -> ScoringEngine {
    let mut engine = ScoringEngine::new();

    engine.add_factor("time_of_day".to_string(), 1.0, Box::new(TimeOfDayScorer));
    engine.add_factor("duration".to_string(), 0.8, Box::new(DurationScorer::new()));
    engine.add_factor("energy".to_string(), 1.2, Box::new(EnergyScorer));
    engine.add_factor("attention".to_string(), 1.2, Box::new(AttentionScorer));
    engine.add_factor("urgency".to_string(), 1.5, Box::new(UrgencyScorer));

    engine
}

// ============================================================================
// Database Integration
// ============================================================================

/// Build a `ScoringContext` from current database state.
///
/// This fetches:
/// - Current time (always fresh)
/// - Latest context snapshot (environment, location, energy, attention)
/// - Current mental state (most recent recorded state)
pub fn build_scoring_context(conn: &Connection) -> Result<ScoringContext> {
    let mut context = ScoringContext::basic();

    // Fetch the current context snapshot if available
    if let Some(snapshot) = crate::context::fetch_current_context(conn)? {
        // Parse energy and attention from metadata JSON
        if let Some(metadata_str) = &snapshot.metadata {
            if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(metadata_str) {
                if let Some(energy) = metadata.get("energy").and_then(|v| v.as_f64()) {
                    context.energy_level = Some(energy.clamp(0.0, 1.0));
                }
                if let Some(attention) = metadata.get("attention").and_then(|v| v.as_f64()) {
                    context.attention_capacity = Some(attention.clamp(0.0, 1.0));
                }
            }
        }

        // Add environment tags (if stored as comma-separated or single value)
        if let Some(env) = snapshot.environment {
            for tag in env.split(',').map(str::trim) {
                if !tag.is_empty() {
                    context.environment.push(tag.to_string());
                }
            }
        }

        // Add location tags
        if let Some(loc) = snapshot.location {
            for tag in loc.split(',').map(str::trim) {
                if !tag.is_empty() {
                    context.location.push(tag.to_string());
                }
            }
        }
    }

    // Fetch the current mental state if available
    if let Some(mental_state) = crate::context::fetch_current_mental_state(conn)? {
        context.mental_state = Some(mental_state.name);
    }

    Ok(context)
}

/// Score a specific instance using current database context.
///
/// Returns the scored instance with full factor breakdown.
pub fn score_instance_with_context(conn: &Connection, instance_id: &str) -> Result<ScoredInstance> {
    // Fetch the instance
    let instances = crate::instance::fetch_instances(conn)?;
    let instance = instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| anyhow::anyhow!("Instance '{}' not found", instance_id))?;

    // Fetch the associated action
    let actions = crate::action::fetch_actions(conn)?;
    let action = actions
        .iter()
        .find(|a| a.id == instance.action_id)
        .ok_or_else(|| anyhow::anyhow!("Action '{}' not found", instance.action_id))?;

    // Build scoring context from current database state
    let context = build_scoring_context(conn)?;

    // Score the instance
    let engine = default_scoring_engine();
    let scored = engine.score_instance(instance, action, &context)?;

    Ok(scored)
}

/// Suggest the best instances to do right now.
///
/// Fetches all available instances (status = "scheduled" or "pending"),
/// scores them using current context, and returns the top N sorted by score.
pub fn suggest_best_instances(
    conn: &Connection,
    count: usize,
) -> Result<Vec<(Instance, Action, f64)>> {
    // Fetch all instances
    let instances = crate::instance::fetch_instances(conn)?;

    // Filter to available instances (scheduled or pending)
    let available: Vec<_> = instances
        .into_iter()
        .filter(|i| i.status == "scheduled" || i.status == "pending")
        .collect();

    // Fetch all actions
    let actions = crate::action::fetch_actions(conn)?;

    // Build scoring context
    let context = build_scoring_context(conn)?;

    // Score all available instances
    let engine = default_scoring_engine();
    let mut scored: Vec<_> = available
        .into_iter()
        .filter_map(|instance| {
            // Find the associated action
            let action = actions.iter().find(|a| a.id == instance.action_id)?;

            // Score it
            let scored = engine.score_instance(&instance, action, &context).ok()?;

            Some((instance, action.clone(), scored.total_score))
        })
        .collect();

    // Sort by score (highest first)
    scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Return top N
    Ok(scored.into_iter().take(count).collect())
}

/// Score all items in a pipeline based on current context.
///
/// Returns a vector of (PipelineItem, score) tuples, preserving the original
/// pipeline order. This allows the caller to either display scores alongside
/// the existing order or re-sort by score.
pub fn score_pipeline_items(
    conn: &Connection,
    pipeline_id: &str,
) -> Result<Vec<(crate::pipeline::PipelineItem, f64)>> {
    // Fetch all pipeline items
    let pipeline_items = crate::pipeline::fetch_pipeline_items(conn, pipeline_id)?;

    // Fetch all actions for lookup
    let actions = crate::action::fetch_actions(conn)?;

    // Build scoring context
    let context = build_scoring_context(conn)?;
    let engine = default_scoring_engine();

    // Score each pipeline item
    let mut scored: Vec<_> = Vec::new();

    for item in pipeline_items {
        // Skip items without an instance_id
        let instance_id = match &item.instance_id {
            Some(id) => id,
            None => continue,
        };

        // Fetch the instance
        let instances = crate::instance::fetch_instances(conn)?;
        let instance = match instances.iter().find(|i| &i.id == instance_id) {
            Some(inst) => inst,
            None => continue,
        };

        // Find the associated action
        let action = match actions.iter().find(|a| a.id == instance.action_id) {
            Some(act) => act,
            None => continue,
        };

        // Score it
        match engine.score_instance(instance, action, &context) {
            Ok(scored_result) => {
                scored.push((item, scored_result.total_score));
            }
            Err(_) => continue,
        }
    }

    Ok(scored)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    struct TestScorer {
        name: String,
        fixed_score: f64,
    }

    impl Scorer for TestScorer {
        fn score(&self, _: &Instance, _: &Action, _: &ScoringContext) -> Result<f64> {
            Ok(self.fixed_score)
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_scoring_engine_combines_factors() {
        let mut engine = ScoringEngine::new();
        engine.add_factor(
            "factor1".to_string(),
            1.0,
            Box::new(TestScorer {
                name: "factor1".to_string(),
                fixed_score: 0.8,
            }),
        );
        engine.add_factor(
            "factor2".to_string(),
            2.0,
            Box::new(TestScorer {
                name: "factor2".to_string(),
                fixed_score: 0.6,
            }),
        );

        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default();
        let context = ScoringContext::basic();

        let result = engine.score_instance(&instance, &action, &context).unwrap();

        // (0.8 * 1.0 + 0.6 * 2.0) / (1.0 + 2.0) = 2.0 / 3.0 ≈ 0.667
        assert!((result.total_score - 0.667).abs() < 0.01);
        assert_eq!(result.factor_scores.len(), 2);
    }

    #[test]
    fn test_scoring_engine_disables_factors() {
        let mut engine = ScoringEngine::new();
        engine.add_factor(
            "factor1".to_string(),
            1.0,
            Box::new(TestScorer {
                name: "factor1".to_string(),
                fixed_score: 0.8,
            }),
        );
        engine.add_factor(
            "factor2".to_string(),
            1.0,
            Box::new(TestScorer {
                name: "factor2".to_string(),
                fixed_score: 0.6,
            }),
        );

        engine.set_factor_enabled("factor2", false);

        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default();
        let context = ScoringContext::basic();

        let result = engine.score_instance(&instance, &action, &context).unwrap();

        // Only factor1 should contribute
        assert!((result.total_score - 0.8).abs() < 0.01);
        assert_eq!(result.factor_scores.len(), 1);
    }

    #[test]
    fn test_time_of_day_from_hour() {
        assert_eq!(TimeOfDay::from_hour(6), TimeOfDay::Morning);
        assert_eq!(TimeOfDay::from_hour(12), TimeOfDay::Afternoon);
        assert_eq!(TimeOfDay::from_hour(18), TimeOfDay::Evening);
        assert_eq!(TimeOfDay::from_hour(22), TimeOfDay::Night);
        assert_eq!(TimeOfDay::from_hour(2), TimeOfDay::Night);
    }

    #[test]
    fn test_scoring_context_builder() {
        let context = ScoringContext::basic()
            .with_energy_level(0.7)
            .with_attention_capacity(0.8)
            .with_mental_state("focused".to_string())
            .with_environment("quiet".to_string())
            .with_location("home".to_string());

        assert_eq!(context.energy_level, Some(0.7));
        assert_eq!(context.attention_capacity, Some(0.8));
        assert_eq!(context.mental_state, Some("focused".to_string()));
        assert_eq!(context.environment, vec!["quiet"]);
        assert_eq!(context.location, vec!["home"]);
    }

    // ========================================================================
    // Phase 1 Scorer Tests
    // ========================================================================

    #[test]
    fn test_time_of_day_scorer_exact_match() {
        let scorer = TimeOfDayScorer;
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default().with_preferred_time_of_day(vec!["morning".to_string()]);
        let context = ScoringContext::basic();

        // Context is created with current time, so we need to check what it is
        let score = scorer.score(&instance, &action, &context).unwrap();

        // Score should be 1.0 if current time matches, or less if not
        assert!(score >= 0.0 && score <= 1.0);
    }

    #[test]
    fn test_time_of_day_scorer_no_preference() {
        let scorer = TimeOfDayScorer;
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default();
        let context = ScoringContext::basic();

        let score = scorer.score(&instance, &action, &context).unwrap();
        assert_eq!(score, 0.5); // Neutral
    }

    #[test]
    fn test_duration_scorer_no_duration() {
        let scorer = DurationScorer::new();
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default();
        let context = ScoringContext::basic();

        let score = scorer.score(&instance, &action, &context).unwrap();
        assert_eq!(score, 0.5); // Neutral
    }

    #[test]
    fn test_duration_scorer_with_available_time() {
        let scorer = DurationScorer::with_available_time(30);
        let instance = Instance::new(Uuid::new_v4());

        // Short task (5 minutes)
        let action = Action::default().duration_bucket(4); // bucket 4 = 5 min
        let context = ScoringContext::basic();

        let score = scorer.score(&instance, &action, &context).unwrap();
        assert_eq!(score, 1.0); // Fits comfortably
    }

    #[test]
    fn test_duration_scorer_too_long() {
        let scorer = DurationScorer::with_available_time(20);
        let instance = Instance::new(Uuid::new_v4());

        // Long task (34 minutes)
        let action = Action::default().duration_bucket(8); // bucket 8 = 34 min
        let context = ScoringContext::basic();

        let score = scorer.score(&instance, &action, &context).unwrap();
        assert_eq!(score, 0.0); // Too long
    }

    #[test]
    fn test_energy_scorer_sufficient_energy() {
        let scorer = EnergyScorer;
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default().energy_rate(2); // Medium energy
        let context = ScoringContext::basic().with_energy_level(0.8); // 80% energy

        let score = scorer.score(&instance, &action, &context).unwrap();
        assert_eq!(score, 1.0); // Sufficient energy
    }

    #[test]
    fn test_energy_scorer_insufficient_energy() {
        let scorer = EnergyScorer;
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default().energy_rate(5); // High energy requirement
        let context = ScoringContext::basic().with_energy_level(0.3); // 30% energy

        let score = scorer.score(&instance, &action, &context).unwrap();
        assert!(score < 0.5); // Low score due to insufficient energy
    }

    #[test]
    fn test_energy_scorer_no_energy_info() {
        let scorer = EnergyScorer;
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default().energy_rate(3);
        let context = ScoringContext::basic(); // No energy level set

        let score = scorer.score(&instance, &action, &context).unwrap();
        assert_eq!(score, 0.5); // Neutral
    }

    #[test]
    fn test_attention_scorer_sufficient_attention() {
        let scorer = AttentionScorer;
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default().attention_level(2); // Medium attention
        let context = ScoringContext::basic().with_attention_capacity(0.9); // 90% capacity

        let score = scorer.score(&instance, &action, &context).unwrap();
        assert_eq!(score, 1.0); // Sufficient attention
    }

    #[test]
    fn test_attention_scorer_insufficient_attention() {
        let scorer = AttentionScorer;
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default().attention_level(5); // High attention requirement
        let context = ScoringContext::basic().with_attention_capacity(0.3); // 30% capacity

        let score = scorer.score(&instance, &action, &context).unwrap();
        assert_eq!(score, 0.2); // Low score
    }

    #[test]
    fn test_urgency_scorer_important_task() {
        let scorer = UrgencyScorer;
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default().importance(5); // Very important
        let context = ScoringContext::basic();

        let score = scorer.score(&instance, &action, &context).unwrap();
        assert_eq!(score, 1.0); // Max importance
    }

    #[test]
    fn test_urgency_scorer_unimportant_task() {
        let scorer = UrgencyScorer;
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default(); // No importance set
        let context = ScoringContext::basic();

        let score = scorer.score(&instance, &action, &context).unwrap();
        assert_eq!(score, 0.3); // Low score for unimportant
    }

    #[test]
    fn test_urgency_scorer_grows_over_time() {
        let scorer = UrgencyScorer;
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default().importance(3).urgency_growth(true);

        // Instance is new, so age boost should be minimal
        let context = ScoringContext::basic();
        let score = scorer.score(&instance, &action, &context).unwrap();

        // Base score from importance (3/5 = 0.6) + minimal age boost
        assert!(score >= 0.6 && score < 0.7);
    }

    #[test]
    fn test_default_scoring_engine_has_all_factors() {
        let engine = default_scoring_engine();
        assert_eq!(engine.factors.len(), 5);

        let factor_names: Vec<_> = engine.factors.iter().map(|f| f.name.as_str()).collect();
        assert!(factor_names.contains(&"time_of_day"));
        assert!(factor_names.contains(&"duration"));
        assert!(factor_names.contains(&"energy"));
        assert!(factor_names.contains(&"attention"));
        assert!(factor_names.contains(&"urgency"));
    }

    #[test]
    fn test_scoring_engine_scores_multiple_instances() {
        let engine = default_scoring_engine();

        let instance1 = Instance::new(Uuid::new_v4());
        let action1 = Action::default().importance(5).energy_rate(1);

        let instance2 = Instance::new(Uuid::new_v4());
        let action2 = Action::default().importance(1).energy_rate(5);

        let context = ScoringContext::basic().with_energy_level(0.9);

        let items = vec![(instance1, action1), (instance2, action2)];
        let scored = engine.score_instances(items, &context).unwrap();

        assert_eq!(scored.len(), 2);
        // First action should score higher (high importance, low energy requirement with high energy available)
        assert!(scored[0].total_score >= scored[1].total_score);
    }

    #[test]
    fn test_scored_instance_explain() {
        let engine = default_scoring_engine();
        let instance = Instance::new(Uuid::new_v4());
        let action = Action::default().importance(3).energy_rate(2);
        let context = ScoringContext::basic().with_energy_level(0.8);

        let scored = engine.score_instance(&instance, &action, &context).unwrap();
        let explanation = scored.explain();

        assert!(explanation.contains("Total score"));
        assert!(explanation.contains("Breakdown:"));
        assert!(explanation.contains("energy"));
        assert!(explanation.contains("urgency"));
    }
}

// Helper trait for testing
#[allow(dead_code)]
trait ActionTestExt {
    fn with_preferred_time_of_day(self, times: Vec<String>) -> Self;
}

impl ActionTestExt for Action {
    fn with_preferred_time_of_day(mut self, times: Vec<String>) -> Self {
        self.preferred_time_of_day = Some(serde_json::to_string(&times).unwrap());
        self
    }
}
