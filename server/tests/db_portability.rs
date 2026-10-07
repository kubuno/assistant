//! Runs the assistant module's own migrations and its delta / array-column
//! plumbing against a real server of **each** engine, from a single compiled
//! binary — the proof that the engine is a run-time choice, not a build-time
//! one, and that the two recently-ported primitives (delta sync and the JSON
//! array column) behave identically on all of them.
//!
//! The module's write paths live in axum handlers that need a full `AppState`
//! (provider set, HTTP clients, …); rather than stand that up, this test drives
//! the very same DB operations the handlers perform — through `kubuno_db`, the
//! module's `sync` helpers and its public `agent_access` reader — so the schema,
//! the journal and the array column are all exercised end to end.
//!
//! * SQLite always runs (a temp file, no server).
//! * PostgreSQL runs when `KUBUNO_PG_TEST_URL` points at a throwaway database.
//! * MySQL/MariaDB runs when `KUBUNO_MYSQL_TEST_URL` does.
//!
//! ```sh
//! KUBUNO_PG_TEST_URL=postgres://u:p@127.0.0.1:5433/assistant \
//! KUBUNO_MYSQL_TEST_URL=mysql://u:p@127.0.0.1:3307/assistant \
//!   SQLX_OFFLINE=true cargo test --test db_portability
//! ```

use kubuno_assistant::handlers::agent_access;
use kubuno_assistant::{sync, SCHEMA};
use kubuno_db::{journal, params};
use uuid::Uuid;

fn base_settings(engine: &str) -> kubuno_db::DbSettings {
    kubuno_db::DbSettings {
        engine: engine.to_string(),
        url: None,
        host: None,
        port: None,
        user: None,
        password: None,
        database: None,
        path: None,
        max_connections: 4,
        min_connections: 0,
        connect_timeout: std::time::Duration::from_secs(10),
        run_migrations: true,
        schema_prefix: None,
    }
}

/// Migrations run one at a time: the PostgreSQL and MySQL suites may share a server.
static EXCLUSIVE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn migrated_pool(settings: kubuno_db::DbSettings) -> (kubuno_db::DbPool, impl Sized) {
    let guard = EXCLUSIVE.lock().await;
    let pool = kubuno_db::connect(&settings, SCHEMA).await.expect("connect");

    kubuno_db::migrations!(
        "./migrations/postgres",
        "./migrations/mysql",
        "./migrations/sqlite",
    )
    .run(&pool, SCHEMA)
    .await
    .expect("migrations");

    kubuno_db::events::ensure_outbox(&pool, SCHEMA).await.expect("outbox");

    (pool, guard)
}

/// The greatest conversation-domain change_seq an owner can currently see.
async fn max_conv_seq(pool: &kubuno_db::DbPool, owner: Uuid) -> i64 {
    let changes = journal::changes_since(
        pool, sync::CONVERSATIONS_TABLE, sync::CONV_TOMBSTONES, owner, 0, 10_000,
    )
    .await
    .expect("conv delta");
    changes.iter().map(|c| c.change_seq).max().unwrap_or(0)
}

/// Inserts a conversation the way `create_conversation` does: a fresh key, a
/// journal seq bound into the row.
async fn create_conversation(pool: &kubuno_db::DbPool, owner: Uuid, title: &str) -> Uuid {
    let id = kubuno_db::new_id();
    let mut tx = pool.begin().await.expect("begin");
    let seq = sync::next_conv_seq(&mut tx).await.expect("seq");
    tx.execute(
        "INSERT INTO assistant.conversations (id, owner_id, title, model_id, provider, change_seq) \
         VALUES ($1, $2, $3, $4, $5, $6)",
        params![id, owner, title, "llama3.2:3b", "ollama", seq],
    )
    .await
    .expect("insert conversation");
    tx.commit().await.expect("commit");
    id
}

/// Inserts a message and bumps its conversation's stats + delta (as
/// `persist_message` does), returning the new message id.
async fn insert_message(
    pool: &kubuno_db::DbPool,
    conv: Uuid,
    role: &str,
    content: &str,
    prompt_tokens: i32,
    completion_tokens: i32,
) -> Uuid {
    let id = kubuno_db::new_id();
    let now = chrono::Utc::now();
    let mut tx = pool.begin().await.expect("begin");
    let seq = sync::next_conv_seq(&mut tx).await.expect("seq");
    tx.execute(
        "INSERT INTO assistant.messages (id, conversation_id, role, content, prompt_tokens, completion_tokens, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        params![id, conv, role, content, prompt_tokens, completion_tokens, now],
    )
    .await
    .expect("insert message");
    tx.execute(
        "UPDATE assistant.conversations SET message_count = message_count + 1, \
             total_tokens = total_tokens + $1 + $2, updated_at = $3, change_seq = $4 WHERE id = $5",
        params![prompt_tokens, completion_tokens, now, seq, conv],
    )
    .await
    .expect("bump conversation");
    tx.commit().await.expect("commit");
    id
}

async fn full_suite(pool: &kubuno_db::DbPool) {
    let user = Uuid::new_v4();

    // ── the migration seeds the three shared system agents and four providers ──
    let sys_agents: i64 = pool
        .fetch_scalar::<i64>(
            "SELECT COUNT(*) FROM assistant.agents WHERE is_system = true",
            params![],
        )
        .await
        .expect("count system agents");
    assert_eq!(sys_agents, 3, "three system agents seeded");
    let providers: i64 = pool
        .fetch_scalar::<i64>("SELECT COUNT(*) FROM assistant.provider_config", params![])
        .await
        .expect("count providers");
    assert_eq!(providers, 4, "four provider rows seeded");

    // ── conversations: create + prove strict change_seq monotonicity ──
    let mut seqs: Vec<i64> = vec![max_conv_seq(pool, user).await]; // 0, none yet

    let a = create_conversation(pool, user, "Alpha").await;
    seqs.push(max_conv_seq(pool, user).await);
    let b = create_conversation(pool, user, "Beta").await;
    seqs.push(max_conv_seq(pool, user).await);

    // a message bumps its conversation's stats + change_seq (child → parent).
    insert_message(pool, a, "user", "hello", 0, 0).await;
    seqs.push(max_conv_seq(pool, user).await);
    insert_message(pool, a, "assistant", "hi there", 5, 7).await;
    seqs.push(max_conv_seq(pool, user).await);

    // the stats really moved (the retired trigger's job, now done in Rust).
    let (count, tokens): (i32, i32) = pool
        .fetch_one_as::<(i32, i32)>(
            "SELECT message_count, total_tokens FROM assistant.conversations WHERE id = $1",
            params![a],
        )
        .await
        .expect("conversation stats");
    assert_eq!(count, 2, "two messages counted");
    assert_eq!(tokens, 12, "prompt+completion tokens accumulated");

    // delete B → a conversation tombstone with a fresh (greater) change_seq.
    {
        let mut tx = pool.begin().await.expect("begin");
        let seq = sync::next_conv_seq(&mut tx).await.expect("seq");
        tx.execute("DELETE FROM assistant.conversations WHERE id = $1", params![b])
            .await
            .expect("delete B");
        journal::record_tombstone(&mut tx, sync::CONV_TOMBSTONES, b, user, seq)
            .await
            .expect("tombstone");
        tx.commit().await.expect("commit");
    }
    seqs.push(max_conv_seq(pool, user).await);

    // EVERY step advanced the sequence: strict monotonicity of next_seq.
    for w in seqs.windows(2) {
        assert!(w[1] > w[0], "conversation change_seq must strictly increase: {seqs:?}");
    }

    // the deletion surfaces as a tombstone; the live one as a modified row.
    let changes = journal::changes_since(
        pool, sync::CONVERSATIONS_TABLE, sync::CONV_TOMBSTONES, user, 0, 10_000,
    )
    .await
    .expect("delta");
    assert!(changes.iter().any(|c| c.id == b && c.deleted), "deleted conversation B is a tombstone");
    assert!(changes.iter().any(|c| c.id == a && !c.deleted), "live conversation A is a modified row");

    // ── folders: a folder deletion detaches its conversations AND bumps them ──
    let folder = {
        let id = kubuno_db::new_id();
        let now = chrono::Utc::now();
        let mut tx = pool.begin().await.expect("begin");
        let seq = sync::next_folder_seq(&mut tx).await.expect("seq");
        tx.execute(
            "INSERT INTO assistant.folders (id, owner_id, name, position, change_seq, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            params![id, user, "Work", 0, seq, now, now],
        )
        .await
        .expect("insert folder");
        // move conversation A into it.
        let cseq = sync::next_conv_seq(&mut tx).await.expect("seq");
        tx.execute(
            "UPDATE assistant.conversations SET folder_id = $1, change_seq = $2 WHERE id = $3",
            params![id, cseq, a],
        )
        .await
        .expect("attach A");
        tx.commit().await.expect("commit");
        id
    };
    let conv_seq_before_delete = max_conv_seq(pool, user).await;

    // delete folder: detach + bump A, folder tombstone (the cascade path).
    {
        let mut tx = pool.begin().await.expect("begin");
        let fseq = sync::next_folder_seq(&mut tx).await.expect("seq");
        let cseq = sync::next_conv_seq(&mut tx).await.expect("seq");
        tx.execute(
            "UPDATE assistant.conversations SET folder_id = NULL, change_seq = $1 WHERE folder_id = $2 AND owner_id = $3",
            params![cseq, folder, user],
        )
        .await
        .expect("detach");
        tx.execute("DELETE FROM assistant.folders WHERE id = $1 AND owner_id = $2", params![folder, user])
            .await
            .expect("delete folder");
        journal::record_tombstone(&mut tx, sync::FOLDER_TOMBSTONES, folder, user, fseq)
            .await
            .expect("folder tombstone");
        tx.commit().await.expect("commit");
    }
    let folder_changes = journal::changes_since(
        pool, sync::FOLDERS_TABLE, sync::FOLDER_TOMBSTONES, user, 0, 10_000,
    )
    .await
    .expect("folder delta");
    assert!(folder_changes.iter().any(|c| c.id == folder && c.deleted), "deleted folder is a tombstone");
    // `folder_id` is a NULL column of a present row, so it is read as
    // `Option<Uuid>` from a typed row (a scalar decode would try to parse NULL).
    let (folder_id_now,): (Option<Uuid>,) = pool
        .fetch_one_as::<(Option<Uuid>,)>("SELECT folder_id FROM assistant.conversations WHERE id = $1", params![a])
        .await
        .expect("read folder_id");
    assert!(folder_id_now.is_none(), "conversation A was detached from the deleted folder");
    assert!(
        max_conv_seq(pool, user).await > conv_seq_before_delete,
        "detaching bumped the conversation so the client learns folder_id became NULL"
    );

    // ── agents + the JSON array column (enabled_tools) ──
    let agent = {
        let id = kubuno_db::new_id();
        let now = chrono::Utc::now();
        let mut tx = pool.begin().await.expect("begin");
        let seq = sync::next_agent_seq(&mut tx).await.expect("seq");
        tx.execute(
            "INSERT INTO assistant.agents (id, name, system_prompt, owner_id, enabled_tools, change_seq, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            params![id, "My Agent", "be helpful", user, vec!["drive.search".to_string(), "calendar.list".to_string()], seq, now, now],
        )
        .await
        .expect("insert agent");
        tx.commit().await.expect("commit");
        id
    };

    // read the array column back through the module's own reader (#[sqlx(json)]).
    let prompt = agent_access::load_prompt(pool, agent, user)
        .await
        .expect("load_prompt")
        .expect("reachable");
    assert_eq!(prompt.system_prompt, "be helpful");
    assert_eq!(
        prompt.enabled_tools,
        vec!["drive.search".to_string(), "calendar.list".to_string()],
        "the JSON array column round-trips"
    );

    // filter by a member with the portable json_array_contains.
    let frag = pool.backend().json_array_contains("enabled_tools", 1);
    let matched: Vec<(Uuid,)> = pool
        .fetch_all_as::<(Uuid,)>(
            &format!("SELECT id FROM assistant.agents WHERE {frag}"),
            params!["drive.search"],
        )
        .await
        .expect("json_array_contains");
    assert!(matched.iter().any(|(id,)| *id == agent), "found the agent by an enabled tool");
    let none: Vec<(Uuid,)> = pool
        .fetch_all_as::<(Uuid,)>(
            &format!("SELECT id FROM assistant.agents WHERE {frag}"),
            params!["nope.nothing"],
        )
        .await
        .expect("json_array_contains miss");
    assert!(none.is_empty(), "an unrelated tool matches nothing");

    // delete the agent → an agent tombstone.
    {
        let mut tx = pool.begin().await.expect("begin");
        let seq = sync::next_agent_seq(&mut tx).await.expect("seq");
        tx.execute("DELETE FROM assistant.agents WHERE id = $1 AND owner_id = $2", params![agent, user])
            .await
            .expect("delete agent");
        journal::record_tombstone(&mut tx, sync::AGENT_TOMBSTONES, agent, user, seq)
            .await
            .expect("agent tombstone");
        tx.commit().await.expect("commit");
    }
    let agent_tombs = journal::changes_since(
        pool, sync::AGENTS_TABLE, sync::AGENT_TOMBSTONES, user, 0, 10_000,
    )
    .await
    .expect("agent delta");
    assert!(agent_tombs.iter().any(|c| c.id == agent && c.deleted), "deleted agent is a tombstone");
    // the reader no longer reaches it.
    assert!(
        agent_access::load_prompt(pool, agent, user).await.expect("load_prompt").is_none(),
        "a deleted agent is unreachable"
    );
}

#[tokio::test]
async fn sqlite_from_the_one_binary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut s = base_settings("sqlite");
    s.path = Some(dir.path().to_string_lossy().into_owned());
    let (pool, _keep) = migrated_pool(s).await;
    full_suite(&pool).await;
}

#[tokio::test]
async fn postgres_from_the_one_binary() {
    let Ok(url) = std::env::var("KUBUNO_PG_TEST_URL") else {
        eprintln!("skipping: KUBUNO_PG_TEST_URL not set");
        return;
    };
    let mut s = base_settings("postgres");
    s.url = Some(url);
    let (pool, _keep) = migrated_pool(s).await;
    full_suite(&pool).await;
}

#[tokio::test]
async fn mysql_from_the_one_binary() {
    let Ok(url) = std::env::var("KUBUNO_MYSQL_TEST_URL") else {
        eprintln!("skipping: KUBUNO_MYSQL_TEST_URL not set");
        return;
    };
    let mut s = base_settings("mysql");
    s.url = Some(url);
    let (pool, _keep) = migrated_pool(s).await;
    full_suite(&pool).await;
}
