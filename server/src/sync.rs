//! Delta-sync plumbing shared by the handlers.
//!
//! The local-first pull (conversations / folders / agents) rests on a monotonic
//! `change_seq` per record and a tombstone per deletion. On PostgreSQL that used
//! to be a `SEQUENCE` plus `BEFORE UPDATE` / `AFTER DELETE` triggers (migration
//! 000006); here it is the portable [`kubuno_db::journal`] primitive, driven
//! from Rust at every write site. This module holds the literal table / domain
//! names those calls take — all `&'static str`, never request data — so the
//! write sites read uniformly and a rename happens in one place.
//!
//! Three entities are versioned: **conversations**, **folders** and **agents**.
//! Messages carry no sequence of their own; every write to a message bumps its
//! parent **conversation** instead (via [`touch_conversation`]), which is what
//! the old `trg_msg_bump_conv` / stats triggers did in the database.

use uuid::Uuid;

/// One shared counter table per schema; `next_seq` keys it by domain.
pub const CHANGE_COUNTER: &str = "assistant.change_counter";

pub const CONVERSATIONS_TABLE: &str = "assistant.conversations";
pub const FOLDERS_TABLE: &str = "assistant.folders";
pub const AGENTS_TABLE: &str = "assistant.agents";

pub const CONV_TOMBSTONES: &str = "assistant.conv_tombstones";
pub const FOLDER_TOMBSTONES: &str = "assistant.folder_tombstones";
pub const AGENT_TOMBSTONES: &str = "assistant.agent_tombstones";

/// Logical counter domains (the row keys in `change_counter`).
pub const CONV_DOMAIN: &str = "conversations";
pub const FOLDER_DOMAIN: &str = "folders";
pub const AGENT_DOMAIN: &str = "agents";

/// The next monotonic sequence for the **conversations** domain, inside `tx`.
pub async fn next_conv_seq(tx: &mut kubuno_db::DbTx) -> Result<i64, sqlx::Error> {
    kubuno_db::journal::next_seq(tx, CHANGE_COUNTER, CONV_DOMAIN).await
}

/// The next monotonic sequence for the **folders** domain, inside `tx`.
pub async fn next_folder_seq(tx: &mut kubuno_db::DbTx) -> Result<i64, sqlx::Error> {
    kubuno_db::journal::next_seq(tx, CHANGE_COUNTER, FOLDER_DOMAIN).await
}

/// The next monotonic sequence for the **agents** domain, inside `tx`.
pub async fn next_agent_seq(tx: &mut kubuno_db::DbTx) -> Result<i64, sqlx::Error> {
    kubuno_db::journal::next_seq(tx, CHANGE_COUNTER, AGENT_DOMAIN).await
}

/// Bumps a **conversation** to a fresh sequence — the portable replacement for
/// the old `trg_msg_bump_conv` no-op trigger. Called after any message write
/// (insert, feedback update, delete, regenerate) so a synchronising client sees
/// the conversation change.
pub async fn touch_conversation(
    tx: &mut kubuno_db::DbTx,
    conversation_id: Uuid,
) -> Result<(), sqlx::Error> {
    kubuno_db::journal::touch(
        tx,
        CONVERSATIONS_TABLE,
        CHANGE_COUNTER,
        CONV_DOMAIN,
        "id",
        conversation_id,
    )
    .await
    .map(|_| ())
}
