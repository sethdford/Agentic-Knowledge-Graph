use graph::{
    Config,
    graph::{Graph, new_graph},
    types::{Node, Edge, NodeId, EdgeId, EntityType, Properties, TemporalRange, Timestamp},
};
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::test]
#[cfg_attr(not(feature = "integration-tests"), ignore)]
async fn test_graph_operations() {
    // Create test configuration
    let config = Config::for_testing();
    
    // Initialize graph
    let graph = new_graph(&config).await.expect("Failed to create graph");
    
    // Create test nodes
    let now = Timestamp(Utc::now());
    let node1 = Node {
        id: NodeId(Uuid::new_v4()),
        entity_type: EntityType::Custom("episode".to_string()),
        label: "test_node_1".to_string(),
        properties: Properties(HashMap::new()),
        valid_time: TemporalRange {
            start: Some(now),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(now),
            end: None,
        },
    };
    
    let node2 = Node {
        id: NodeId(Uuid::new_v4()),
        entity_type: EntityType::Custom("semantic_entity".to_string()),
        label: "test_node_2".to_string(),
        properties: Properties(HashMap::new()),
        valid_time: TemporalRange {
            start: Some(now),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(now),
            end: None,
        },
    };
    
    // Test node creation
    let node1_id = graph.create_node(node1.clone()).await.expect("Failed to create node 1");
    let node2_id = graph.create_node(node2.clone()).await.expect("Failed to create node 2");
    
    assert_eq!(node1_id, node1.id);
    assert_eq!(node2_id, node2.id);
    
    // Test node retrieval
    let retrieved_node1 = graph.get_node(node1_id).await.expect("Failed to get node 1");
    assert_eq!(retrieved_node1.id, node1.id);
    assert_eq!(retrieved_node1.label, node1.label);
    
    // Create test edge
    let edge = Edge {
        id: EdgeId(Uuid::new_v4()),
        source_id: node1_id,
        target_id: node2_id,
        label: "test_edge".to_string(),
        properties: Properties(HashMap::new()),
        valid_time: TemporalRange {
            start: Some(now),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(now),
            end: None,
        },
    };
    
    // Test edge creation
    let edge_id = graph.create_edge(edge.clone()).await.expect("Failed to create edge");
    assert_eq!(edge_id, edge.id);
    
    // Test edge retrieval
    let retrieved_edge = graph.get_edge(edge_id).await.expect("Failed to get edge");
    assert_eq!(retrieved_edge.id, edge.id);
    assert_eq!(retrieved_edge.label, edge.label);
    
    // Test getting connected nodes
    let connected_nodes = graph.get_connected_nodes(node1_id, None)
        .await
        .expect("Failed to get connected nodes");
    assert_eq!(connected_nodes.len(), 1);
    assert_eq!(connected_nodes[0].id, node2_id);
    
    // Test getting edges for node
    let node_edges = graph.get_edges_for_node(node1_id, None)
        .await
        .expect("Failed to get edges for node");
    assert_eq!(node_edges.len(), 1);
    assert_eq!(node_edges[0].id, edge_id);
    
    // Test temporal filtering
    let future_time = TemporalRange {
        start: Some(Timestamp(Utc::now() + chrono::Duration::hours(1))),
        end: Some(Timestamp(Utc::now() + chrono::Duration::hours(2))),
    };
    
    let future_edges = graph.get_edges_for_node(node1_id, Some(future_time))
        .await
        .expect("Failed to get future edges");
    assert_eq!(future_edges.len(), 0);
    
    // Test node deletion
    graph.delete_node(node1_id).await.expect("Failed to delete node 1");
    let node1_result = graph.get_node(node1_id).await;
    assert!(node1_result.is_err());
    
    // Test edge deletion
    graph.delete_edge(edge_id).await.expect("Failed to delete edge");
    let edge_result = graph.get_edge(edge_id).await;
    assert!(edge_result.is_err());
} 