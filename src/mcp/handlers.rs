use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use serde_json::json;
use uuid::Uuid;

use crate::{
    api::state::ApiState,
    error::{Error, Result},
    graph::{Edge, Node},
    temporal::Temporal,
    types::{EntityId, EntityType},
};
use super::{MCPRequest, MCPResponse, CursorPosition};

pub async fn handle_mcp_request(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<MCPRequest>,
) -> impl IntoResponse {
    match request {
        MCPRequest::Navigate { entity_id, direction } => {
            Json(MCPResponse {
                request_id: Uuid::new_v4().to_string(),
                status: "success".to_string(),
                data: json!({
                    "message": format!("Navigation to {} not implemented yet", direction),
                    "entity_id": entity_id
                }),
            })
        },
        MCPRequest::EntityDetail { entity_id } => {
            Json(MCPResponse {
                request_id: Uuid::new_v4().to_string(),
                status: "success".to_string(),
                data: json!({
                    "message": "Entity detail not implemented yet",
                    "entity_id": entity_id
                }),
            })
        },
        MCPRequest::Command { request_id, command, data: request_data, cursor_position } => {
            // Original stub implementation
            Json(MCPResponse {
                request_id,
                status: "success".to_string(),
                data: json!({
                    "message": "MCP handler stub implementation",
                    "command": command
                }),
            })
        }
    }
}

async fn handle_get_references(state: Arc<ApiState>, request: MCPRequest) -> MCPResponse {
    let (request_id, cursor_pos) = match &request {
        MCPRequest::Navigate { entity_id, direction } => (
            Uuid::new_v4().to_string(),
            None
        ),
        MCPRequest::EntityDetail { entity_id } => (
            Uuid::new_v4().to_string(),
            None
        ),
        MCPRequest::Command { request_id, command, data, cursor_position } => (
            request_id.clone(),
            cursor_position.as_ref()
        ),
    };
    
    let cursor_pos = match cursor_pos {
        Some(pos) => pos,
        None => &CursorPosition::default(),
    };
    
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
                request_id,
                status: "success".to_string(),
                data: json!({
                    "references": node.properties.get("references").unwrap_or(&json!([])),
                    "timestamp": result.timestamp,
                }),
            }
        },
        _ => MCPResponse {
            request_id,
            status: "error".to_string(),
            data: json!({"error": "No references found"}),
        }
    }
}

async fn handle_get_definition(state: Arc<ApiState>, request: MCPRequest) -> MCPResponse {
    let (request_id, cursor_pos) = match &request {
        MCPRequest::Navigate { entity_id, direction } => (
            Uuid::new_v4().to_string(),
            None
        ),
        MCPRequest::EntityDetail { entity_id } => (
            Uuid::new_v4().to_string(),
            None
        ),
        MCPRequest::Command { request_id, command, data, cursor_position } => (
            request_id.clone(),
            cursor_position.as_ref()
        ),
    };
    
    let cursor_pos = match cursor_pos {
        Some(pos) => pos,
        None => &CursorPosition::default(),
    };
    
    let entity_id = EntityId {
        entity_type: EntityType::Node,
        id: format!("{}:{}:{}", cursor_pos.file, cursor_pos.line, cursor_pos.column),
    };
    
    match state.temporal.query_latest(&entity_id).await {
        Ok(Some(result)) => {
            let node: Node = result.data;
            MCPResponse {
                request_id,
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
            request_id,
            status: "error".to_string(),
            data: json!({"error": "Definition not found"}),
        }
    }
}

async fn handle_get_implementations(state: Arc<ApiState>, request: MCPRequest) -> MCPResponse {
    let (request_id, cursor_pos) = match &request {
        MCPRequest::Navigate { entity_id, direction } => (
            Uuid::new_v4().to_string(),
            None
        ),
        MCPRequest::EntityDetail { entity_id } => (
            Uuid::new_v4().to_string(),
            None
        ),
        MCPRequest::Command { request_id, command, data, cursor_position } => (
            request_id.clone(),
            cursor_position.as_ref()
        ),
    };
    
    let cursor_pos = match cursor_pos {
        Some(pos) => pos,
        None => &CursorPosition::default(),
    };
    
    let entity_id = EntityId {
        entity_type: EntityType::Node,
        id: format!("{}:{}:{}", cursor_pos.file, cursor_pos.line, cursor_pos.column),
    };
    
    match state.temporal.query_latest(&entity_id).await {
        Ok(Some(result)) => {
            let node: Node = result.data;
            MCPResponse {
                request_id,
                status: "success".to_string(),
                data: json!({
                    "implementations": node.properties.get("implementations").unwrap_or(&json!([])),
                    "timestamp": result.timestamp,
                }),
            }
        },
        _ => MCPResponse {
            request_id,
            status: "error".to_string(),
            data: json!({"error": "No implementations found"}),
        }
    }
}

async fn handle_type_hierarchy(state: Arc<ApiState>, request: MCPRequest) -> MCPResponse {
    let (request_id, cursor_pos) = match &request {
        MCPRequest::Navigate { entity_id, direction } => (
            Uuid::new_v4().to_string(),
            None
        ),
        MCPRequest::EntityDetail { entity_id } => (
            Uuid::new_v4().to_string(),
            None
        ),
        MCPRequest::Command { request_id, command, data, cursor_position } => (
            request_id.clone(),
            cursor_position.as_ref()
        ),
    };
    
    let cursor_pos = match cursor_pos {
        Some(pos) => pos,
        None => &CursorPosition::default(),
    };
    
    let entity_id = EntityId {
        entity_type: EntityType::Node,
        id: format!("{}:{}:{}", cursor_pos.file, cursor_pos.line, cursor_pos.column),
    };
    
    match state.temporal.query_latest(&entity_id).await {
        Ok(Some(result)) => {
            let node: Node = result.data;
            MCPResponse {
                request_id,
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
            request_id,
            status: "error".to_string(),
            data: json!({"error": "Type hierarchy not found"}),
        }
    }
}

async fn handle_unknown_command(request: MCPRequest) -> MCPResponse {
    let request_id = match &request {
        MCPRequest::Navigate { .. } => Uuid::new_v4().to_string(),
        MCPRequest::EntityDetail { .. } => Uuid::new_v4().to_string(),
        MCPRequest::Command { request_id, .. } => request_id.clone(),
    };
    
    MCPResponse {
        request_id,
        status: "error".to_string(),
        data: json!({"error": "Unknown command"}),
    }
}

async fn navigate_entity(
    State(state): State<Arc<ApiState>>,
    Path(entity_id): Path<String>,
    Path(cmd): Path<String>,
) -> impl IntoResponse {
    // Stub implementation
    // This is a temporary fix to get the code to compile
    Json(MCPResponse {
        request_id: Uuid::new_v4().to_string(),
        status: "success".to_string(),
        data: json!({
            "message": "Navigation not implemented yet",
            "entity_id": entity_id,
            "command": cmd
        }),
    })
}

async fn get_entity(
    State(state): State<Arc<ApiState>>,
    Path(entity_id): Path<String>,
) -> impl IntoResponse {
    // Stub implementation
    // This is a temporary fix to get the code to compile
    Json(MCPResponse {
        request_id: Uuid::new_v4().to_string(),
        status: "success".to_string(),
        data: json!({
            "message": "Entity retrieval not implemented yet",
            "entity_id": entity_id
        }),
    })
}

pub async fn query_mcp(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<MCPRequest>,
) -> impl IntoResponse {
    match request {
        MCPRequest::Navigate { entity_id, direction } => {
            Json(MCPResponse {
                request_id: Uuid::new_v4().to_string(),
                status: "success".to_string(),
                data: json!({
                    "message": format!("Navigation to {} not implemented yet", direction),
                    "entity_id": entity_id
                }),
            })
        }
        MCPRequest::EntityDetail { entity_id } => {
            Json(MCPResponse {
                request_id: Uuid::new_v4().to_string(),
                status: "success".to_string(),
                data: json!({
                    "message": "Entity detail not implemented yet",
                    "entity_id": entity_id
                }),
            })
        }
        MCPRequest::Command { request_id, command, data, cursor_position } => {
            Json(MCPResponse {
                request_id,
                status: "success".to_string(),
                data: json!({
                    "message": "Command processing not implemented yet",
                    "command": command
                }),
            })
        }
    }
} 