use graph::{
    api::{create_router, models::*},
    Config,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
#[ignore] // Ignored by default - can be run with `cargo test -- --ignored`
async fn test_api_endpoints() {
    // Create test configuration
    let config = Config::for_testing();
    
    // Create the API router with testing state
    let app = create_router();
    
    // ---- Test health endpoint ----
    let health_response = app
        .clone()
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    
    assert_eq!(health_response.status(), StatusCode::OK);
    
    let health_body = hyper::body::to_bytes(health_response.into_body()).await.unwrap();
    let health_json: Value = serde_json::from_slice(&health_body).unwrap();
    
    assert_eq!(health_json["status"], "healthy");
    assert!(health_json["version"].is_string());
    assert!(health_json["uptime"].is_number());
    assert!(health_json["components"].is_array());
    
    // ---- Test version endpoint ----
    let version_response = app
        .clone()
        .oneshot(Request::builder().uri("/version").body(Body::empty()).unwrap())
        .await
        .unwrap();
    
    assert_eq!(version_response.status(), StatusCode::OK);
    
    let version_body = hyper::body::to_bytes(version_response.into_body()).await.unwrap();
    let version_json: Value = serde_json::from_slice(&version_body).unwrap();
    
    assert!(version_json["version"].is_string());
    assert!(version_json["build_date"].is_string());
    
    // ---- Test create node endpoint ----
    let node_id = Uuid::new_v4();
    let create_node_json = json!({
        "id": node_id.to_string(),
        "entity_type": "Person",
        "label": "Test Person",
        "properties": {
            "name": "Test Person",
            "age": 30
        },
        "valid_time": {
            "start": Utc::now().to_rfc3339(),
            "end": null
        }
    });
    
    let create_node_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/nodes")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(create_node_json.to_string()))
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(create_node_response.status(), StatusCode::CREATED);
    
    let create_node_body = hyper::body::to_bytes(create_node_response.into_body()).await.unwrap();
    let created_node: Value = serde_json::from_slice(&create_node_body).unwrap();
    
    assert_eq!(created_node["id"], node_id.to_string());
    assert_eq!(created_node["entity_type"], "Person");
    assert_eq!(created_node["label"], "Test Person");
    
    // ---- Test get node endpoint ----
    let get_node_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/nodes/{}", node_id))
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(get_node_response.status(), StatusCode::OK);
    
    let get_node_body = hyper::body::to_bytes(get_node_response.into_body()).await.unwrap();
    let retrieved_node: Value = serde_json::from_slice(&get_node_body).unwrap();
    
    assert_eq!(retrieved_node["id"], node_id.to_string());
    assert_eq!(retrieved_node["entity_type"], "Person");
    assert_eq!(retrieved_node["label"], "Test Person");
    assert_eq!(retrieved_node["properties"]["name"], "Test Person");
    assert_eq!(retrieved_node["properties"]["age"], 30);
    
    // ---- Test create edge endpoint ----
    let target_node_id = Uuid::new_v4();
    
    // First create target node
    let target_node_json = json!({
        "id": target_node_id.to_string(),
        "entity_type": "Organization",
        "label": "Test Organization",
        "properties": {
            "name": "Test Organization",
            "founded": 2010
        },
        "valid_time": {
            "start": Utc::now().to_rfc3339(),
            "end": null
        }
    });
    
    let create_target_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/nodes")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(target_node_json.to_string()))
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(create_target_response.status(), StatusCode::CREATED);
    
    // Now create edge between nodes
    let edge_id = Uuid::new_v4();
    let create_edge_json = json!({
        "id": edge_id.to_string(),
        "source_id": node_id.to_string(),
        "target_id": target_node_id.to_string(),
        "label": "WORKS_AT",
        "properties": {
            "role": "Developer",
            "since": 2020
        },
        "valid_time": {
            "start": Utc::now().to_rfc3339(),
            "end": null
        }
    });
    
    let create_edge_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/edges")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(create_edge_json.to_string()))
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(create_edge_response.status(), StatusCode::CREATED);
    
    let create_edge_body = hyper::body::to_bytes(create_edge_response.into_body()).await.unwrap();
    let created_edge: Value = serde_json::from_slice(&create_edge_body).unwrap();
    
    assert_eq!(created_edge["id"], edge_id.to_string());
    assert_eq!(created_edge["label"], "WORKS_AT");
    assert_eq!(created_edge["source_id"], node_id.to_string());
    assert_eq!(created_edge["target_id"], target_node_id.to_string());
    
    // ---- Test get edge endpoint ----
    let get_edge_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/edges/{}", edge_id))
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(get_edge_response.status(), StatusCode::OK);
    
    let get_edge_body = hyper::body::to_bytes(get_edge_response.into_body()).await.unwrap();
    let retrieved_edge: Value = serde_json::from_slice(&get_edge_body).unwrap();
    
    assert_eq!(retrieved_edge["id"], edge_id.to_string());
    assert_eq!(retrieved_edge["label"], "WORKS_AT");
    assert_eq!(retrieved_edge["properties"]["role"], "Developer");
    
    // ---- Test query knowledge endpoint ----
    let query_json = json!({
        "filter": {
            "entity_type": "Person",
            "properties": {
                "age": {
                    "operator": ">=",
                    "value": 25
                }
            }
        },
        "include_edges": true,
        "time_point": Utc::now().to_rfc3339()
    });
    
    let query_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/knowledge/query")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(query_json.to_string()))
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(query_response.status(), StatusCode::OK);
    
    let query_body = hyper::body::to_bytes(query_response.into_body()).await.unwrap();
    let query_result: Value = serde_json::from_slice(&query_body).unwrap();
    
    assert!(query_result["results"].is_array());
    assert!(query_result["metadata"].is_object());
    
    // ---- Test update node endpoint ----
    let update_node_json = json!({
        "id": node_id.to_string(),
        "entity_type": "Person",
        "label": "Updated Test Person",
        "properties": {
            "name": "Updated Test Person",
            "age": 31,
            "updated": true
        },
        "valid_time": {
            "start": Utc::now().to_rfc3339(),
            "end": null
        }
    });
    
    let update_node_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/nodes/{}", node_id))
                .method("PUT")
                .header("Content-Type", "application/json")
                .body(Body::from(update_node_json.to_string()))
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(update_node_response.status(), StatusCode::OK);
    
    let update_node_body = hyper::body::to_bytes(update_node_response.into_body()).await.unwrap();
    let updated_node: Value = serde_json::from_slice(&update_node_body).unwrap();
    
    assert_eq!(updated_node["label"], "Updated Test Person");
    assert_eq!(updated_node["properties"]["age"], 31);
    assert_eq!(updated_node["properties"]["updated"], true);
    
    // ---- Test delete node endpoints ----
    // Delete edge first to avoid constraints
    let delete_edge_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/edges/{}", edge_id))
                .method("DELETE")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(delete_edge_response.status(), StatusCode::OK);
    
    // Now delete nodes
    let delete_node_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/nodes/{}", node_id))
                .method("DELETE")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(delete_node_response.status(), StatusCode::OK);
    
    let delete_target_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/nodes/{}", target_node_id))
                .method("DELETE")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(delete_target_response.status(), StatusCode::OK);
    
    // ---- Test error handling for non-existent node ----
    let nonexistent_id = Uuid::new_v4();
    let get_nonexistent_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/nodes/{}", nonexistent_id))
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(get_nonexistent_response.status(), StatusCode::NOT_FOUND);
    
    let error_body = hyper::body::to_bytes(get_nonexistent_response.into_body()).await.unwrap();
    let error_json: Value = serde_json::from_slice(&error_body).unwrap();
    
    assert_eq!(error_json["error"], "Not Found");
    assert!(error_json["message"].is_string());
} 