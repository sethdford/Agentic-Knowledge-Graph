use axum::{
    extract::State,
    response::{IntoResponse, Json},
};
use std::sync::Arc;
use uuid::Uuid;
use serde_json::json;

use crate::{
    api::ApiState,
    error::{Error, Result},
};
use super::{MCPRequest, MCPResponse};

/// Handle mcp request
#[axum::debug_handler]
pub async fn handle_mcp(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<MCPRequest>,
) -> impl IntoResponse {
    match process_request(state, request).await {
        Ok(response) => Json(response),
        Err(e) => {
            // Return error response
            let error_response = MCPResponse::Error { 
                error: format!("Error processing MCP request: {}", e),
            };
            Json(error_response)
        }
    }
}

async fn process_request(_state: Arc<ApiState>, request: MCPRequest) -> Result<MCPResponse> {
    match request {
        MCPRequest::Command { request_id, command, data: request_data, cursor_position: _cursor_position } => {
            match command.as_str() {
                "get_entity" => {
                    let entity_id = match request_data.get("id") {
                        Some(id) => id.as_str().ok_or(Error::InvalidInput("Invalid entity ID".into()))?,
                        None => return Ok(MCPResponse::Error { error: "Missing entity ID".to_string() }),
                    };
                    
                    // Since the ApiState doesn't have a temporal field, we'll return a mock response
                    // In a real implementation, you would use the actual temporal system
                    Ok(MCPResponse::Entity { 
                        request_id,
                        entity: json!({
                            "id": entity_id,
                            "type": "node", 
                            "properties": {},
                            "note": "This is a mock response as ApiState doesn't have temporal field"
                        })
                    })
                },
                "get_connections" => {
                    let entity_id = match request_data.get("id") {
                        Some(id) => id.as_str().ok_or(Error::InvalidInput("Invalid entity ID".into()))?,
                        None => return Ok(MCPResponse::Error { error: "Missing entity ID".to_string() }),
                    };

                    // Return mock response
                    Ok(MCPResponse::Connections { 
                        request_id,
                        connections: json!([
                            {
                                "id": Uuid::new_v4().to_string(),
                                "type": "RELATED_TO",
                                "source": entity_id,
                                "target": Uuid::new_v4().to_string(),
                                "properties": {}
                            }
                        ])
                    })
                },
                "get_entity_history" => {
                    let entity_id = match request_data.get("id") {
                        Some(id) => id.as_str().ok_or(Error::InvalidInput("Invalid entity ID".into()))?,
                        None => return Ok(MCPResponse::Error { error: "Missing entity ID".to_string() }),
                    };

                    // Return mock response
                    Ok(MCPResponse::History { 
                        request_id,
                        history: json!([
                            {
                                "id": entity_id,
                                "timestamp": "2023-05-01T12:00:00Z",
                                "type": "node",
                                "properties": { "name": "Initial state" }
                            },
                            {
                                "id": entity_id,
                                "timestamp": "2023-05-02T15:30:00Z", 
                                "type": "node",
                                "properties": { "name": "Updated state" }
                            }
                        ])
                    })
                },
                "search_entities" => {
                    let query = match request_data.get("query") {
                        Some(q) => q.as_str().ok_or(Error::InvalidInput("Invalid query".into()))?,
                        None => return Ok(MCPResponse::Error { error: "Missing query".to_string() }),
                    };

                    // Return mock search results
                    Ok(MCPResponse::SearchResults { 
                        request_id,
                        results: json!([
                            {
                                "id": Uuid::new_v4().to_string(),
                                "type": "node",
                                "score": 0.95,
                                "properties": { "name": format!("Result for '{}'", query) }
                            },
                            {
                                "id": Uuid::new_v4().to_string(),
                                "type": "node", 
                                "score": 0.82,
                                "properties": { "name": format!("Another match for '{}'", query) }
                            }
                        ])
                    })
                },
                "create_entity" => {
                    // Mock entity creation
                    let id = Uuid::new_v4().to_string();
                    
                    Ok(MCPResponse::EntityCreated { 
                        request_id,
                        id: id.clone(),
                        entity: json!({
                            "id": id,
                            "type": "node",
                            "properties": request_data.get("properties").unwrap_or(&json!({}))
                        })
                    })
                },
                _ => Ok(MCPResponse::Error { 
                    error: format!("Unknown command: {}", command) 
                })
            }
        },
        MCPRequest::Query { request_id, query_text } => {
            // Mock query response
            Ok(MCPResponse::QueryResult { 
                request_id,
                result: json!({
                    "query": query_text,
                    "matches": [
                        {
                            "id": Uuid::new_v4().to_string(),
                            "relevance": 0.89,
                            "content": "Mock result for your query"
                        }
                    ]
                })
            })
        },
        MCPRequest::Cancel { request_id } => {
            // Just acknowledge the cancellation
            Ok(MCPResponse::Cancelled { request_id })
        },
        MCPRequest::Navigate { entity_id, direction } => {
            // Return mock navigation result
            Ok(MCPResponse::Entity {
                request_id: Uuid::new_v4().to_string(),
                entity: json!({
                    "id": entity_id,
                    "direction": direction,
                    "type": "node",
                    "note": "Navigation mock result"
                })
            })
        },
        MCPRequest::EntityDetail { entity_id } => {
            // Return mock entity detail
            Ok(MCPResponse::Entity {
                request_id: Uuid::new_v4().to_string(),
                entity: json!({
                    "id": entity_id,
                    "type": "node",
                    "properties": {
                        "name": "Mock Entity",
                        "created_at": chrono::Utc::now().to_rfc3339()
                    },
                    "note": "Entity detail mock result"
                })
            })
        }
    }
}

/// Get MCP status
#[axum::debug_handler]
pub async fn get_status(
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    // Return status information
    Json(json!({
        "status": "operational", 
        "uptime": state.uptime().as_secs(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Get MCP config
#[axum::debug_handler]
pub async fn get_config(
    State(_state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    // Return configuration (mock)
    Json(json!({
        "max_history_items": 100,
        "default_search_limit": 20,
        "features": {
            "history_tracking": true,
            "semantic_search": true,
            "real_time_updates": false
        }
    }))
}

/// Update MCP config
#[axum::debug_handler]
pub async fn update_config(
    State(_state): State<Arc<ApiState>>,
    Json(config): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Pretend to update config and return response
    Json(json!({
        "status": "success",
        "message": "Configuration updated",
        "config": config
    }))
}

/// Execute MCP command
#[axum::debug_handler]
pub async fn execute_command(
    State(_state): State<Arc<ApiState>>,
    Json(request): Json<MCPRequest>,
) -> impl IntoResponse {
    match request {
        MCPRequest::Command { request_id, command, data: _data, cursor_position: _cursor_position } => {
            // Mock command execution response
            Json(json!({
                "status": "success",
                "request_id": request_id,
                "command": command,
                "result": {
                    "message": "Command executed successfully",
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }
            }))
        },
        _ => Json(json!({
            "status": "error",
            "message": "Invalid request type, expected Command"
        }))
    }
} 