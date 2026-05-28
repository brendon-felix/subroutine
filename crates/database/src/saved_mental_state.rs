use anyhow::{Context, Result};
use app_core::{starter_states, SavedMentalState};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Flat database representation of a SavedMentalState.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedMentalStateModel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub attention_mode: i64,
    pub sensory_tolerance: i64,
    pub emotional_regulation: i64,
    pub social_battery: i64,
}

impl From<&SavedMentalState> for SavedMentalStateModel {
    fn from(state: &SavedMentalState) -> Self {
        Self {
            id: state.id.to_string(),
            name: state.name.clone(),
            description: state.description.clone(),
            attention_mode: state.attention_mode as i64,
            sensory_tolerance: state.sensory_tolerance as i64,
            emotional_regulation: state.emotional_regulation as i64,
            social_battery: state.social_battery as i64,
        }
    }
}

impl TryFrom<SavedMentalStateModel> for SavedMentalState {
    type Error = anyhow::Error;

    fn try_from(model: SavedMentalStateModel) -> Result<Self> {
        let id = Uuid::parse_str(&model.id)
            .with_context(|| format!("Invalid saved_mental_state id '{}'", model.id))?;

        Ok(SavedMentalState {
            id,
            name: model.name,
            description: model.description,
            attention_mode: model.attention_mode as i8,
            sensory_tolerance: model.sensory_tolerance as i8,
            emotional_regulation: model.emotional_regulation as i8,
            social_battery: model.social_battery as i8,
        })
    }
}

fn row_to_model(row: &rusqlite::Row) -> rusqlite::Result<SavedMentalStateModel> {
    Ok(SavedMentalStateModel {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        attention_mode: row.get(3)?,
        sensory_tolerance: row.get(4)?,
        emotional_regulation: row.get(5)?,
        social_battery: row.get(6)?,
    })
}

pub fn insert_saved_mental_state(conn: &Connection, state: &SavedMentalState) -> Result<()> {
    let model = SavedMentalStateModel::from(state);
    conn.execute(
        r#"
            INSERT INTO saved_mental_states (
                id, name, description,
                attention_mode, sensory_tolerance, emotional_regulation, social_battery
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                attention_mode = excluded.attention_mode,
                sensory_tolerance = excluded.sensory_tolerance,
                emotional_regulation = excluded.emotional_regulation,
                social_battery = excluded.social_battery
        "#,
        rusqlite::params![
            model.id,
            model.name,
            model.description,
            model.attention_mode,
            model.sensory_tolerance,
            model.emotional_regulation,
            model.social_battery,
        ],
    )
    .context("Failed to insert or update saved mental state")?;
    Ok(())
}

pub fn fetch_saved_mental_states(conn: &Connection) -> Result<Vec<SavedMentalState>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, name, description,
                   attention_mode, sensory_tolerance, emotional_regulation, social_battery
            FROM saved_mental_states
            ORDER BY name ASC
            "#,
        )
        .context("Failed to prepare saved mental state fetch query")?;

    let states = stmt
        .query_map([], |row| row_to_model(row))
        .context("Failed to query saved mental states")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to map saved mental state rows")?
        .into_iter()
        .map(SavedMentalState::try_from)
        .collect::<Result<Vec<_>>>()
        .context("Failed to convert saved mental state models")?;

    Ok(states)
}

pub fn fetch_saved_mental_state_by_id(
    conn: &Connection,
    id: Uuid,
) -> Result<Option<SavedMentalState>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, name, description,
                   attention_mode, sensory_tolerance, emotional_regulation, social_battery
            FROM saved_mental_states
            WHERE id = $1
            "#,
        )
        .context("Failed to prepare saved mental state fetch by id query")?;

    let model = stmt
        .query_row([id.to_string()], |row| row_to_model(row))
        .optional()
        .context("Failed to fetch saved mental state by id")?;

    model.map(SavedMentalState::try_from).transpose()
}

pub fn delete_saved_mental_state(conn: &Connection, id: Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM saved_mental_states WHERE id = $1",
        [id.to_string()],
    )
    .with_context(|| format!("Failed to delete saved mental state '{}'", id))?;
    Ok(())
}

/// Inserts any starter states that are not already present in the database.
/// Called once on startup. Existing rows (including user-modified versions of
/// starter states) are left untouched.
pub fn seed_starter_mental_states(conn: &Connection) -> Result<()> {
    for state in starter_states::all() {
        let already_exists = conn
            .query_row(
                "SELECT COUNT(*) FROM saved_mental_states WHERE id = $1",
                [state.id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .context("Failed to check for existing starter mental state")?
            > 0;

        if !already_exists {
            insert_saved_mental_state(conn, &state)?;
        }
    }
    Ok(())
}
