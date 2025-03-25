use axum::{
    Router,
    routing::post,
    extract::State,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    api::ApiState,
    types::{Node, Edge, EntityId, EntityType},
};

mod handlers;

#[derive(Debug, Serialize, Deserialize)]
pub enum MCPRequest {
    Navigate {
        entity_id: String,
        direction: String,
    },
    EntityDetail {
        entity_id: String,
    },
    Command {
        request_id: String,
        command: String,
        data: serde_json::Value,
        cursor_position: Option<CursorPosition>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CursorPosition {
    file: String,
    line: u32,
    column: u32,
}

impl Default for CursorPosition {
    fn default() -> Self {
        Self {
            file: String::new(),
            line: 0,
            column: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MCPResponse {
    request_id: String,
    status: String,
    data: serde_json::Value,
}

pub fn mcp_router(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/mcp/graph", post(handlers::handle_mcp_request))
        .with_state(state)
} 