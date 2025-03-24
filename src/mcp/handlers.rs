use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use serde_json::json;
use uuid::Uuid;

use crate::{
    api::ApiState,
    types::{EntityId, EntityType, Node},
    temporal::{TemporalIndex, Temporal},
};

use super::{MCPRequest, MCPResponse, CursorPosition};

pub async fn handle_mcp_request(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<MCPRequest>,
) -> impl IntoResponse {
    let response = match request.command.as_str() {
        "get_references" => handle_get_references(state, request).await,
        "get_definition" => handle_get_definition(state, request).await,
        "get_implementations" => handle_get_implementations(state, request).await,
        "get_type_hierarchy" => handle_type_hierarchy(state, request).await,
        _ => handle_unknown_command(request).await,
    };
    
    Json(response)
}

async fn handle_get_references(state: Arc<ApiState>, request: MCPRequest) -> MCPResponse {
    let cursor_pos = request.cursor_position.unwrap_or_default();
    
    // Create an entity ID from the cursor position
    let entity_id = EntityId {
        entity_type: EntityType::Node,
        id: format!("{}:{}:{}", cursor_pos.file, cursor_pos.line, cursor_pos.column),
    };
    
    // Query the temporal graph for references
    match state.temporal.query_latest(&entity_id).await {
        Ok(Some(result)) => {
            let node: Node = result.data;
            MCPResponse {
                request_id: request.request_id,
                status: "success".to_string(),
                data: json!({
                    "references": node.properties.get("references").unwrap_or(&json!([])),
                    "timestamp": result.timestamp,
                }),
            }
        },
        _ => MCPResponse {
            request_id: request.request_id,
            status: "error".to_string(),
            data: json!({"error": "No references found"}),
        }
    }
}

async fn handle_get_definition(state: Arc<ApiState>, request: MCPRequest) -> MCPResponse {
    let cursor_pos = request.cursor_position.unwrap_or_default();
    
    let entity_id = EntityId {
        entity_type: EntityType::Node,
        id: format!("{}:{}:{}", cursor_pos.file, cursor_pos.line, cursor_pos.column),
    };
    
    match state.temporal.query_latest(&entity_id).await {
        Ok(Some(result)) => {
            let node: Node = result.data;
            MCPResponse {
                request_id: request.request_id,
                status: "success".to_string(),
                data: json!({
                    "definition": {
                        "file": node.properties.get("definition_file"),
                        "line": node.properties.get("definition_line"),
                        "column": node.properties.get("definition_column"),
                    },
                    "timestamp": result.timestamp,
                }),
            }
        },
        _ => MCPResponse {
            request_id: request.request_id,
            status: "error".to_string(),
            data: json!({"error": "Definition not found"}),
        }
    }
}

async fn handle_get_implementations(state: Arc<ApiState>, request: MCPRequest) -> MCPResponse {
    let cursor_pos = request.cursor_position.unwrap_or_default();
    
    let entity_id = EntityId {
        entity_type: EntityType::Node,
        id: format!("{}:{}:{}", cursor_pos.file, cursor_pos.line, cursor_pos.column),
    };
    
    match state.temporal.query_latest(&entity_id).await {
        Ok(Some(result)) => {
            let node: Node = result.data;
            MCPResponse {
                request_id: request.request_id,
                status: "success".to_string(),
                data: json!({
                    "implementations": node.properties.get("implementations").unwrap_or(&json!([])),
                    "timestamp": result.timestamp,
                }),
            }
        },
        _ => MCPResponse {
            request_id: request.request_id,
            status: "error".to_string(),
            data: json!({"error": "No implementations found"}),
        }
    }
}

async fn handle_type_hierarchy(state: Arc<ApiState>, request: MCPRequest) -> MCPResponse {
    let cursor_pos = request.cursor_position.unwrap_or_default();
    
    let entity_id = EntityId {
        entity_type: EntityType::Node,
        id: format!("{}:{}:{}", cursor_pos.file, cursor_pos.line, cursor_pos.column),
    };
    
    match state.temporal.query_latest(&entity_id).await {
        Ok(Some(result)) => {
            let node: Node = result.data;
            MCPResponse {
                request_id: request.request_id,
                status: "success".to_string(),
                data: json!({
                    "hierarchy": {
                        "parents": node.properties.get("parent_types").unwrap_or(&json!([])),
                        "children": node.properties.get("child_types").unwrap_or(&json!([])),
                    },
                    "timestamp": result.timestamp,
                }),
            }
        },
        _ => MCPResponse {
            request_id: request.request_id,
            status: "error".to_string(),
            data: json!({"error": "Type hierarchy not found"}),
        }
    }
}

async fn handle_unknown_command(request: MCPRequest) -> MCPResponse {
    MCPResponse {
        request_id: request.request_id,
        status: "error".to_string(),
        data: json!({"error": "Unknown command"}),
    }
} 