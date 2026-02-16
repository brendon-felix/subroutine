//! Shared utilities for resolving entities from user-provided identifiers.
//!
//! This module provides a common pattern for matching entities by:
//! 1. Exact ID match
//! 2. ID prefix match
//! 3. Case-insensitive name/title prefix match
//!
//! Returns an error if zero or multiple matches are found.

use anyhow::Result;
use rusqlite::Connection;

/// Trait for entities that can be resolved by identifier
pub trait Resolvable: Clone {
    /// Get the entity's unique ID
    fn id(&self) -> &str;

    /// Get the entity's human-readable name/title (used for prefix matching)
    fn name(&self) -> &str;

    /// Fetch all entities of this type from the database
    fn fetch_all(conn: &Connection) -> Result<Vec<Self>>;

    /// Resolve an entity from a user-provided identifier.
    ///
    /// The identifier can be:
    /// - An exact ID match
    /// - An ID prefix
    /// - A case-insensitive name/title prefix
    ///
    /// Returns an error if no matches or multiple ambiguous matches are found.
    fn resolve(conn: &Connection, identifier: &str) -> Result<Self> {
        let entities = Self::fetch_all(conn)?;

        // Try exact ID match first
        if let Some(entity) = entities.iter().find(|e| e.id() == identifier) {
            return Ok(entity.clone());
        }

        // Try ID prefix match
        let id_matches: Vec<_> = entities
            .iter()
            .filter(|e| e.id().starts_with(identifier))
            .collect();

        if id_matches.len() == 1 {
            return Ok(id_matches[0].clone());
        }

        if id_matches.len() > 1 {
            let mut message = format!(
                "Ambiguous ID prefix '{}'. Multiple IDs match:\n",
                identifier
            );
            for entity in &id_matches {
                message.push_str(&format!("  [{}] {}\n", &entity.id()[..8], entity.name()));
            }
            message.push_str("Please use a more specific ID prefix or the full ID.");
            return Err(anyhow::anyhow!(message));
        }

        // Try case-insensitive name/title prefix match
        let name_matches: Vec<_> = entities
            .iter()
            .filter(|e| {
                e.name()
                    .to_lowercase()
                    .starts_with(&identifier.to_lowercase())
            })
            .collect();

        match name_matches.len() {
            0 => Err(anyhow::anyhow!("No entity found matching '{}'", identifier)),
            1 => Ok(name_matches[0].clone()),
            _ => {
                let mut message = format!(
                    "Ambiguous identifier '{}'. Multiple entities match:\n",
                    identifier
                );
                for entity in &name_matches {
                    message.push_str(&format!("  [{}] {}\n", &entity.id()[..8], entity.name()));
                }
                message.push_str("Please use a more specific identifier or the full ID.");
                Err(anyhow::anyhow!(message))
            }
        }
    }
}

// Implement Resolvable for Action
impl Resolvable for database::Action {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.title
    }

    fn fetch_all(conn: &Connection) -> Result<Vec<Self>> {
        database::fetch_actions(conn)
    }
}

// Implement Resolvable for Instance
impl Resolvable for database::Instance {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        // Instances don't have their own name, so we use the action_id as a fallback
        // In practice, callers should consider joining with actions for better UX
        &self.action_id
    }

    fn fetch_all(conn: &Connection) -> Result<Vec<Self>> {
        database::fetch_instances(conn)
    }
}

/// Convenience function for resolving actions
pub fn resolve_action(conn: &Connection, identifier: &str) -> Result<database::Action> {
    database::Action::resolve(conn, identifier)
}

/// Convenience function for resolving instances
pub fn resolve_instance(conn: &Connection, identifier: &str) -> Result<database::Instance> {
    database::Instance::resolve(conn, identifier)
}

// Implement Resolvable for MentalState
impl Resolvable for database::MentalState {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn fetch_all(conn: &Connection) -> Result<Vec<Self>> {
        database::fetch_mental_states(conn)
    }
}

/// Convenience function for resolving mental states
pub fn resolve_mental_state(conn: &Connection, identifier: &str) -> Result<database::MentalState> {
    database::MentalState::resolve(conn, identifier)
}

// Implement Resolvable for PipelineItem
impl Resolvable for database::PipelineItem {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        // Use action_title if available, otherwise fall back to ID
        self.action_title.as_deref().unwrap_or(&self.id)
    }

    fn fetch_all(conn: &Connection) -> Result<Vec<Self>> {
        database::fetch_pipeline_items(conn, database::DEFAULT_PIPELINE_ID)
    }
}

/// Convenience function for resolving pipeline items
#[allow(dead_code)]
pub fn resolve_pipeline_item(
    conn: &Connection,
    identifier: &str,
) -> Result<database::PipelineItem> {
    database::PipelineItem::resolve(conn, identifier)
}

/// Convenience function for resolving pipeline items from a specific pipeline
pub fn resolve_pipeline_item_in(
    conn: &Connection,
    pipeline_id: &str,
    identifier: &str,
) -> Result<database::PipelineItem> {
    let items = database::fetch_pipeline_items(conn, pipeline_id)?;

    // Try exact ID match first
    if let Some(item) = items.iter().find(|i| i.id == identifier) {
        return Ok(item.clone());
    }

    // Try ID prefix match
    let id_matches: Vec<_> = items
        .iter()
        .filter(|i| i.id.starts_with(identifier))
        .collect();

    if id_matches.len() == 1 {
        return Ok(id_matches[0].clone());
    }

    if id_matches.len() > 1 {
        let mut message = format!(
            "Ambiguous ID prefix '{}'. Multiple pipeline items match:\n",
            identifier
        );
        for item in &id_matches {
            let name = item.action_title.as_deref().unwrap_or("(no title)");
            message.push_str(&format!("  [{}] {}\n", &item.id[..8], name));
        }
        message.push_str("Please use a more specific ID prefix or the full ID.");
        return Err(anyhow::anyhow!(message));
    }

    // Try case-insensitive action_title prefix match
    let name_matches: Vec<_> = items
        .iter()
        .filter(|i| {
            if let Some(title) = &i.action_title {
                title.to_lowercase().starts_with(&identifier.to_lowercase())
            } else {
                false
            }
        })
        .collect();

    match name_matches.len() {
        0 => Err(anyhow::anyhow!(
            "No pipeline item found matching '{}'",
            identifier
        )),
        1 => Ok(name_matches[0].clone()),
        _ => {
            let mut message = format!(
                "Ambiguous identifier '{}'. Multiple pipeline items match:\n",
                identifier
            );
            for item in &name_matches {
                let name = item.action_title.as_deref().unwrap_or("(no title)");
                message.push_str(&format!("  [{}] {}\n", &item.id[..8], name));
            }
            message.push_str("Please use a more specific identifier or the full ID.");
            Err(anyhow::anyhow!(message))
        }
    }
}
