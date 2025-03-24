use crate::types::{Node, Edge, NodeId, EdgeId, TemporalRange};
use serde_json::json;

/// Build a Gremlin query to create a node
pub(crate) fn create_node(node: &Node) -> String {
    let properties = json!({
        "id": node.id.0.to_string(),
        "label": node.label,
        "entity_type": node.entity_type.to_string(),
        "properties": node.properties,
        "valid_time": node.valid_time,
        "transaction_time": node.transaction_time,
    });

    format!(
        "g.addV('{}'){}",
        node.entity_type.to_string(),
        properties.as_object().unwrap()
            .iter()
            .map(|(k, v)| format!(".property('{}', {})", k, v.to_string()))
            .collect::<Vec<_>>()
            .join("")
    )
}

/// Build a Gremlin query to get a node by ID
pub(crate) fn get_node(id: NodeId) -> String {
    format!("g.V('{}').hasLabel('node')", id.0)
}

/// Build a Gremlin query to update a node
pub(crate) fn update_node(node: &Node) -> String {
    let properties = json!({
        "label": node.label,
        "properties": node.properties,
        "valid_time": node.valid_time,
        "transaction_time": node.transaction_time,
    });

    format!(
        "g.V('{}'){}",
        node.id.0,
        properties.as_object().unwrap()
            .iter()
            .map(|(k, v)| format!(".property('{}', {})", k, v.to_string()))
            .collect::<Vec<_>>()
            .join("")
    )
}

/// Build a Gremlin query to delete a node
pub(crate) fn delete_node(id: NodeId) -> String {
    format!("g.V('{}').drop()", id.0)
}

/// Build a Gremlin query to create an edge
pub(crate) fn create_edge(edge: &Edge) -> String {
    let properties = json!({
        "id": edge.id.0.to_string(),
        "properties": edge.properties,
        "valid_time": edge.valid_time,
        "transaction_time": edge.transaction_time,
    });

    format!(
        "g.V('{}').addE('{}').to(V('{}')){}",
        edge.source_id.0,
        edge.label,
        edge.target_id.0,
        properties.as_object().unwrap()
            .iter()
            .map(|(k, v)| format!(".property('{}', {})", k, v.to_string()))
            .collect::<Vec<_>>()
            .join("")
    )
}

/// Build a Gremlin query to get an edge by ID
pub(crate) fn get_edge(id: EdgeId) -> String {
    format!("g.E('{}').hasLabel('edge')", id.0)
}

/// Build a Gremlin query to update an edge
pub(crate) fn update_edge(edge: &Edge) -> String {
    let properties = json!({
        "properties": edge.properties,
        "valid_time": edge.valid_time,
        "transaction_time": edge.transaction_time,
    });

    format!(
        "g.E('{}'){}",
        edge.id.0,
        properties.as_object().unwrap()
            .iter()
            .map(|(k, v)| format!(".property('{}', {})", k, v.to_string()))
            .collect::<Vec<_>>()
            .join("")
    )
}

/// Build a Gremlin query to delete an edge
pub(crate) fn delete_edge(id: EdgeId) -> String {
    format!("g.E('{}').drop()", id.0)
}

/// Build a Gremlin query to get edges for a node with temporal filtering
pub(crate) fn get_edges_for_node(node_id: NodeId, temporal_range: Option<TemporalRange>) -> String {
    let mut query = format!("g.V('{}').bothE()", node_id.0);
    
    if let Some(range) = temporal_range {
        if let Some(start) = range.start {
            query.push_str(&format!(".has('valid_time.start', gte({}))", start.0.timestamp()));
        }
        if let Some(end) = range.end {
            query.push_str(&format!(".has('valid_time.end', lte({}))", end.0.timestamp()));
        }
    }
    
    query
}

/// Build a Gremlin query to get connected nodes with temporal filtering
pub(crate) fn get_connected_nodes(node_id: NodeId, temporal_range: Option<TemporalRange>) -> String {
    let mut query = format!("g.V('{}').both()", node_id.0);
    
    if let Some(range) = temporal_range {
        if let Some(start) = range.start {
            query.push_str(&format!(".has('valid_time.start', gte({}))", start.0.timestamp()));
        }
        if let Some(end) = range.end {
            query.push_str(&format!(".has('valid_time.end', lte({}))", end.0.timestamp()));
        }
    }
    
    query
}

/// Build a Gremlin query to get nodes by label
pub(crate) fn get_nodes_by_label(label: &str) -> String {
    format!("g.V().hasLabel('{}')", label)
}

/// Build a Gremlin query to get edges by label
pub(crate) fn get_edges_by_label(label: &str) -> String {
    format!("g.E().hasLabel('{}')", label)
}

/// Build a Gremlin query to get edges between two nodes
pub(crate) fn get_edges_between(from: &NodeId, to: &NodeId) -> String {
    format!("g.V('{}').outE().where(inV().hasId('{}'))", from.0, to.0)
}

/// Build a Gremlin query to get edges from a node
pub(crate) fn get_edges_from(from: &NodeId) -> String {
    format!("g.V('{}').outE()", from.0)
}

/// Build a Gremlin query to get edges to a node
pub(crate) fn get_edges_to(to: &NodeId) -> String {
    format!("g.V('{}').inE()", to.0)
}

/// Build a Gremlin query to get a vertex by ID
pub(crate) fn get_vertex(id: &str) -> String {
    format!("g.V('{}')", id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::types::{EntityType, Properties, Timestamp};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn create_test_node() -> Node {
        let now = Timestamp(Utc::now());
        Node {
            id: NodeId(Uuid::new_v4()),
            entity_type: EntityType::Event,
            label: "test_node".to_string(),
            properties: Properties(HashMap::new()),
            valid_time: TemporalRange {
                start: Some(now),
                end: None,
            },
            transaction_time: TemporalRange {
                start: Some(now),
                end: None,
            },
        }
    }

    fn create_test_edge() -> Edge {
        let now = Timestamp(Utc::now());
        Edge {
            id: EdgeId(Uuid::new_v4()),
            source_id: NodeId(Uuid::new_v4()),
            target_id: NodeId(Uuid::new_v4()),
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
        }
    }

    #[test]
    fn test_create_node_query() {
        let node = create_test_node();
        let query = create_node(&node);
        assert!(query.contains(&node.id.0.to_string()));
        assert!(query.contains(&node.label));
        assert!(query.contains("addV"));
    }

    #[test]
    fn test_create_edge_query() {
        let edge = create_test_edge();
        let query = create_edge(&edge);
        assert!(query.contains(&edge.id.0.to_string()));
        assert!(query.contains(&edge.source_id.0.to_string()));
        assert!(query.contains(&edge.target_id.0.to_string()));
        assert!(query.contains("addE"));
    }

    #[test]
    fn test_temporal_query() {
        let node_id = NodeId(Uuid::new_v4());
        let now = Utc::now();
        let range = TemporalRange {
            start: Some(Timestamp(now)),
            end: Some(Timestamp(now + chrono::Duration::hours(1))),
        };
        
        let query = get_edges_for_node(node_id, Some(range));
        assert!(query.contains(&node_id.0.to_string()));
        assert!(query.contains("valid_time.start"));
        assert!(query.contains("valid_time.end"));
        assert!(query.contains("gte"));
        assert!(query.contains("lte"));
    }
} 