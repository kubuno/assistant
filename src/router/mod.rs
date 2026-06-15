use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    handlers::{agents, conversations, messages, models, settings},
    state::AppState,
};

pub fn build(state: AppState) -> Router {
    let api = Router::new()
        // Models
        .route("/models", get(models::list_models))
        // Provider settings
        .route("/settings/providers",           get(settings::list_providers))
        .route("/settings/providers/:provider", axum::routing::patch(settings::update_provider))
        // Agents
        .route("/agents",     get(agents::list_agents).post(agents::create_agent))
        .route("/agents/:id", get(agents::get_agent).patch(agents::update_agent).delete(agents::delete_agent))
        // Conversations
        .route("/conversations",     get(conversations::list_conversations).post(conversations::create_conversation))
        .route("/conversations/:id", get(conversations::get_conversation).patch(conversations::update_conversation).delete(conversations::delete_conversation))
        .route("/conversations/:id/messages", get(conversations::list_messages))
        // Send message → SSE stream
        .route("/conversations/:id/chat", post(messages::send_message))
        .with_state(state.clone());

    let health = Router::new()
        .route("/health", get(health_handler))
        .with_state(state);

    Router::new()
        .merge(health)
        .nest("/", api)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn health_handler() -> axum::http::StatusCode {
    axum::http::StatusCode::OK
}
