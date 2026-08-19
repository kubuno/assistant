//! Agent access control — the one place that decides which agents a user may
//! read, use or edit.
//!
//! An agent is reachable when the caller owns it, or when it is one of the
//! system agents the module ships with (`owner_id IS NULL`, `is_system = true`)
//! that every account shares. Nothing else grants access: the `is_public` column
//! of `assistant.agents` is dormant — no route sets it and no listing reads it —
//! so it deliberately opens nothing here. The day sharing is implemented, this
//! module is the single file to change.
//!
//! Two rules callers must honour:
//!
//! * an unreachable agent is reported as **not found**, never as forbidden.
//!   Answering "forbidden" for somebody else's agent turns id guessing into an
//!   existence oracle. `Forbidden` is reserved for the system agents, which are
//!   already visible to everyone and are merely read-only;
//! * an agent id coming from a request body is an *access*, not just a value:
//!   it must go through here before it is stored or used, otherwise a
//!   conversation could be pinned to another account's agent and run — and leak
//!   — its `system_prompt`.

use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AssistantError, AssistantResult};

/// Visibility of the whole collection, for listings. `$1` = user id.
pub const VISIBLE_SQL: &str = "(owner_id = $1 OR is_system = true)";

/// Visibility of a single agent. `$1` = agent id, `$2` = user id.
pub const REACHABLE_SQL: &str = "id = $1 AND (owner_id = $2 OR is_system = true)";

/// The same rule expressed in Rust, applied to the row the SQL returned.
///
/// The predicate above already filters, so this is a second gate rather than the
/// only one: a query that ever loses its `WHERE` clause still cannot hand a
/// stranger's agent back.
pub fn is_reachable(owner_id: Option<Uuid>, is_system: bool, user_id: Uuid) -> bool {
    is_system || owner_id == Some(user_id)
}

/// The error every caller returns for an agent it may not touch — identical to
/// the one for an id that does not exist at all.
pub fn not_found() -> AssistantError {
    AssistantError::NotFound("agent introuvable".into())
}

/// What running a conversation on an agent needs from it.
pub struct AgentPrompt {
    pub system_prompt: String,
    pub enabled_tools: Vec<String>,
}

/// Loads the prompt and tool scope of an agent the user may use.
///
/// `Ok(None)` = unknown id **or** somebody else's agent; the caller cannot tell
/// the two apart, and neither can its client.
pub async fn load_prompt(
    db:       &PgPool,
    agent_id: Uuid,
    user_id:  Uuid,
) -> AssistantResult<Option<AgentPrompt>> {
    let sql = format!(
        "SELECT owner_id, is_system, system_prompt, enabled_tools \
         FROM assistant.agents WHERE {REACHABLE_SQL}"
    );
    let row = sqlx::query_as::<_, (Option<Uuid>, bool, String, Vec<String>)>(&sql)
        .bind(agent_id)
        .bind(user_id)
        .fetch_optional(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, agent_id = %agent_id, "chargement de l'agent d'une conversation");
            AssistantError::from(e)
        })?;

    Ok(row
        .filter(|(owner_id, is_system, _, _)| is_reachable(*owner_id, *is_system, user_id))
        .map(|(_, _, system_prompt, enabled_tools)| AgentPrompt { system_prompt, enabled_tools }))
}

/// Whether the user may touch this agent, and whether it is a shared system one.
///
/// `Ok(None)` = unknown or not the caller's; `Ok(Some(true))` = a system agent,
/// which is readable by everyone but writable by no one.
pub async fn reachable(
    db:       &PgPool,
    agent_id: Uuid,
    user_id:  Uuid,
) -> AssistantResult<Option<bool>> {
    let sql = format!(
        "SELECT owner_id, is_system FROM assistant.agents WHERE {REACHABLE_SQL}"
    );
    let row = sqlx::query_as::<_, (Option<Uuid>, bool)>(&sql)
        .bind(agent_id)
        .bind(user_id)
        .fetch_optional(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, agent_id = %agent_id, "vérification d'accès à un agent");
            AssistantError::from(e)
        })?;

    Ok(row
        .filter(|(owner_id, is_system)| is_reachable(*owner_id, *is_system, user_id))
        .map(|(_, is_system)| is_system))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uid(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn an_agent_belonging_to_somebody_else_is_never_reachable() {
        let me    = uid(1);
        let other = uid(2);
        assert!(!is_reachable(Some(other), false, me));
    }

    #[test]
    fn my_own_agent_is_reachable() {
        let me = uid(1);
        assert!(is_reachable(Some(me), false, me));
    }

    #[test]
    fn the_shipped_system_agents_are_shared_with_everyone() {
        // owner_id IS NULL + is_system: the agents the migration seeds.
        assert!(is_reachable(None, true, uid(1)));
        assert!(is_reachable(None, true, uid(2)));
    }

    #[test]
    fn an_ownerless_agent_that_is_not_a_system_one_is_reachable_by_nobody() {
        assert!(!is_reachable(None, false, uid(1)));
    }

    /// Regression guard: the single-agent predicate must keep binding BOTH the
    /// id and the user. Dropping the owner clause is exactly the bug that let a
    /// guessed id hand out another account's `system_prompt`.
    #[test]
    fn the_single_agent_predicate_is_bound_to_the_owner() {
        assert!(REACHABLE_SQL.contains("id = $1"));
        assert!(REACHABLE_SQL.contains("owner_id = $2"));
        assert!(REACHABLE_SQL.contains("is_system = true"));
        assert!(VISIBLE_SQL.contains("owner_id = $1"));
    }

    /// An unreachable agent must look exactly like a missing one.
    #[test]
    fn the_refusal_is_a_not_found_never_a_forbidden() {
        assert!(matches!(not_found(), AssistantError::NotFound(_)));
    }
}
