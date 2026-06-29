//! Provider-agnostic agentic tool-calling loop. Each round: call the model with
//! the tool catalog; if it requests tools, execute each via the core MCP gateway
//! (or hand `kubuno_ui` tools to the client), feed the results back, and loop —
//! until the model returns a final text answer.
//!
//! Message shaping differs per provider (Anthropic uses tool_use/tool_result
//! content blocks; Ollama uses assistant.tool_calls + role:"tool" messages), so
//! each provider implements [`AgenticProvider`]; the loop itself is shared.
//!
//! Non-streaming within the loop (one completion per round): tool-calling needs
//! the full structured response before acting. The final answer is emitted as a
//! single Delta — token-by-token streaming inside the loop is a follow-up.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use axum::async_trait;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::mcp_client::{McpClient, ToolCatalogItem};
use super::provider::LlmMessage;

/// Safety bound on tool rounds per user turn.
const MAX_ITERS: usize = 6;

/// One tool the model asked to call.
#[derive(Debug, Clone)]
pub struct ToolUse {
    pub id:    String,
    pub name:  String,
    pub input: Value,
}

/// Result of one non-streaming completion.
pub struct AgenticTurn {
    pub text:          String,
    pub tool_uses:     Vec<ToolUse>,
    /// The assistant message to append verbatim before the tool results.
    pub assistant_msg: Value,
    pub input_tokens:  i32,
    pub output_tokens: i32,
}

/// Outcome of executing one tool, fed back to the model.
pub struct ToolOutcome<'a> {
    pub tu:       &'a ToolUse,
    pub content:  String,
    pub is_error: bool,
}

/// A chat model capable of tool use. Implemented by each provider.
#[async_trait]
pub trait AgenticProvider: Send + Sync {
    /// Map a catalog tool to this provider's tool schema.
    fn tool_schema(&self, t: &ToolCatalogItem) -> Value;
    /// One completion with the given (provider-shaped) messages + tools.
    async fn complete_once(&self, model: &str, system: &str, messages: &[Value], tools: &[Value]) -> Result<AgenticTurn>;
    /// The message(s) carrying tool results back to the model.
    fn tool_results_turn(&self, outcomes: &[ToolOutcome<'_>]) -> Vec<Value>;
}

/// Event emitted by the agentic loop, consumed by the SSE layer.
#[derive(Debug, Clone)]
pub enum AgenticEvent {
    Delta(String),
    /// A tool call (backend) or UI action (kind=="ui"). Shape:
    /// `{ tool, kind, args, result?, is_error?, ui? }`.
    ToolCall(Value),
    Done { input_tokens: i32, output_tokens: i32 },
    Error(String),
}

/// Spawn the loop and return a receiver of its events.
pub fn run_agentic(
    provider: Arc<dyn AgenticProvider>,
    mcp: McpClient,
    user_id: Uuid,
    model: String,
    system_text: String,
    turns: Vec<LlmMessage>,
    tools: Vec<ToolCatalogItem>,
) -> mpsc::UnboundedReceiver<AgenticEvent> {
    let (tx, rx) = mpsc::unbounded_channel::<AgenticEvent>();

    tokio::spawn(async move {
        let prov_tools: Vec<Value> = tools.iter().map(|t| provider.tool_schema(t)).collect();
        // name → UI action descriptor, for tools dispatched client-side.
        let ui_map: HashMap<String, Value> = tools.iter()
            .filter_map(|t| t.ui_action().map(|ui| (t.name.clone(), ui)))
            .collect();
        // Tools requiring explicit user confirmation are NOT auto-executed: the
        // loop emits a `confirm` card and the client runs them after consent
        // (POST /jarvis/tools/call). Avoids acting before the user agrees.
        let confirm_set: std::collections::HashSet<String> = tools.iter()
            .filter(|t| t.needs_confirm())
            .map(|t| t.name.clone())
            .collect();

        let mut messages: Vec<Value> = turns.iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect();
        let mut total_in  = 0i32;
        let mut total_out = 0i32;

        for _ in 0..MAX_ITERS {
            let turn = match provider.complete_once(&model, &system_text, &messages, &prov_tools).await {
                Ok(t)  => t,
                Err(e) => { let _ = tx.send(AgenticEvent::Error(e.to_string())); return; }
            };
            total_in  += turn.input_tokens;
            total_out += turn.output_tokens;

            if !turn.text.is_empty() {
                let _ = tx.send(AgenticEvent::Delta(turn.text.clone()));
            }

            if turn.tool_uses.is_empty() {
                let _ = tx.send(AgenticEvent::Done { input_tokens: total_in, output_tokens: total_out });
                return;
            }

            messages.push(turn.assistant_msg.clone());

            let mut outcomes: Vec<ToolOutcome> = Vec::new();
            for tu in &turn.tool_uses {
                if let Some(ui) = ui_map.get(&tu.name) {
                    let _ = tx.send(AgenticEvent::ToolCall(json!({
                        "tool": tu.name, "kind": "ui", "args": tu.input, "ui": ui,
                    })));
                    outcomes.push(ToolOutcome { tu, content: "Action transmise à l'interface de l'utilisateur.".into(), is_error: false });
                    continue;
                }
                if confirm_set.contains(&tu.name) {
                    // Defer to the user: emit a confirm card, don't execute now.
                    let _ = tx.send(AgenticEvent::ToolCall(json!({
                        "tool": tu.name, "kind": "confirm", "args": tu.input,
                    })));
                    outcomes.push(ToolOutcome {
                        tu,
                        content: "Cette action requiert la confirmation explicite de l'utilisateur ; elle sera exécutée seulement après son accord. Ne la considère pas comme effectuée.".into(),
                        is_error: false,
                    });
                    continue;
                }
                let (text, is_error) = match mcp.call_tool(user_id, &tu.name, &tu.input).await {
                    Ok((t, e)) => (t, e),
                    Err(e)     => (format!("Erreur d'appel de l'outil: {e}"), true),
                };
                let _ = tx.send(AgenticEvent::ToolCall(json!({
                    "tool": tu.name, "kind": "backend", "args": tu.input,
                    "result": text, "is_error": is_error,
                })));
                outcomes.push(ToolOutcome { tu, content: text, is_error });
            }

            for m in provider.tool_results_turn(&outcomes) {
                messages.push(m);
            }
        }

        let _ = tx.send(AgenticEvent::Done { input_tokens: total_in, output_tokens: total_out });
    });

    rx
}
