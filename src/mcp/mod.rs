use axum::{
    Router,
    routing::{get, post},
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

pub mod handlers;

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
    Query {
        request_id: String,
        query_text: String,
    },
    Cancel {
        request_id: String,
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
#[serde(tag = "type")]
pub enum MCPResponse {
    Error {
        error: String,
    },
    Entity {
        request_id: String,
        entity: serde_json::Value,
    },
    Connections {
        request_id: String,
        connections: serde_json::Value,
    },
    History {
        request_id: String,
        history: serde_json::Value,
    },
    SearchResults {
        request_id: String,
        results: serde_json::Value,
    },
    EntityCreated {
        request_id: String,
        id: String,
        entity: serde_json::Value,
    },
    QueryResult {
        request_id: String,
        result: serde_json::Value,
    },
    Cancelled {
        request_id: String,
    },
}

pub fn mcp_router(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/mcp/graph", post(handlers::handle_mcp))
        .route("/mcp/status", get(handlers::get_status))
        .route("/mcp/config", get(handlers::get_config))
        .route("/mcp/config", post(handlers::update_config))
        .route("/mcp/command", post(handlers::execute_command))
        .with_state(state)
} 