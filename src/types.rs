use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::str::FromStr;
use gremlin_client::{GResultSet, GValue, Vertex, Edge as GremlinEdge, ToGValue};
use serde_json::Value;
use std::fmt;

use crate::error::{Error, Result};

/// Unique identifier for nodes in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for edges in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub Uuid);

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Entity ID wrapper
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
pub struct EntityId {
    pub entity_type: EntityType,
    pub id: String,
}

impl EntityId {
    /// Create a new entity ID
    pub fn new(entity_type: EntityType, id: impl Into<String>) -> Self {
        Self {
            entity_type,
            id: id.into(),
        }
    }
}

impl From<String> for EntityId {
    fn from(s: String) -> Self {
        EntityId {
            entity_type: EntityType::Node,
            id: s,
        }
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// Timestamp wrapper
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Timestamp(pub DateTime<Utc>);

impl Timestamp {
    /// Create a new timestamp with the current time
    pub fn now() -> Self {
        Self(Utc::now())
    }
    
    /// Create a timestamp from a DateTime
    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self(Utc::now())
    }
}

impl Copy for Timestamp {}

/// Temporal range for valid time queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalRange {
    /// Start time (inclusive)
    pub start: Option<Timestamp>,
    /// End time (inclusive)
    pub end: Option<Timestamp>,
}

impl TemporalRange {
    /// Create a new temporal range
    pub fn new(start: Option<Timestamp>, end: Option<Timestamp>) -> Self {
        Self { start, end }
    }

    /// Create a temporal range from the current time onwards
    pub fn from_now() -> Self {
        Self {
            start: Some(Timestamp::now()),
            end: None,
        }
    }

    /// Create an unbounded temporal range (infinite in both directions)
    pub fn unbounded() -> Self {
        Self {
            start: None,
            end: None,
        }
    }

    /// Check if a timestamp is within this range
    pub fn contains(&self, timestamp: &DateTime<Utc>) -> bool {
        let after_start = match &self.start {
            Some(start) => timestamp >= &start.0,
            None => true,
        };
        
        let before_end = match &self.end {
            Some(end) => timestamp <= &end.0,
            None => true,
        };
        
        after_start && before_end
    }
    
    /// Check if this range overlaps with another range
    pub fn overlaps(&self, other: &TemporalRange) -> bool {
        let start_after = match (&self.start, &other.end) {
            (Some(start), Some(end)) => start.0 <= end.0,
            _ => true,
        };
        
        let end_before = match (&self.end, &other.start) {
            (Some(end), Some(start)) => end.0 >= start.0,
            _ => true,
        };
        
        start_after && end_before
    }
}

/// Type of entity in the graph
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
pub enum EntityType {
    Node,
    Edge,
    Person,
    Organization,
    Location,
    Event,
    Topic,
    Document,
    Vertex,
    Date,
    Time,
    Money,
    Percentage,
    Product,
    Other,
    Custom(String),
}

impl Default for EntityType {
    fn default() -> Self {
        Self::Node
    }
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityType::Node => write!(f, "Node"),
            EntityType::Edge => write!(f, "Edge"),
            EntityType::Person => write!(f, "Person"),
            EntityType::Organization => write!(f, "Organization"),
            EntityType::Location => write!(f, "Location"),
            EntityType::Event => write!(f, "Event"),
            EntityType::Topic => write!(f, "Topic"),
            EntityType::Document => write!(f, "Document"),
            EntityType::Vertex => write!(f, "Vertex"),
            EntityType::Date => write!(f, "Date"),
            EntityType::Time => write!(f, "Time"),
            EntityType::Money => write!(f, "Money"),
            EntityType::Percentage => write!(f, "Percentage"),
            EntityType::Product => write!(f, "Product"),
            EntityType::Other => write!(f, "Other"),
            EntityType::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl std::str::FromStr for EntityType {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "node" => Ok(EntityType::Node),
            "edge" => Ok(EntityType::Edge),
            "person" => Ok(EntityType::Person),
            "organization" => Ok(EntityType::Organization),
            "location" => Ok(EntityType::Location),
            "event" => Ok(EntityType::Event),
            "topic" => Ok(EntityType::Topic),
            "document" => Ok(EntityType::Document),
            "vertex" => Ok(EntityType::Vertex),
            "date" => Ok(EntityType::Date),
            "time" => Ok(EntityType::Time),
            "money" => Ok(EntityType::Money),
            "percentage" => Ok(EntityType::Percentage),
            "product" => Ok(EntityType::Product),
            "other" => Ok(EntityType::Other),
            s => Ok(EntityType::Custom(s.to_string())),
        }
    }
}

/// Properties associated with nodes and edges
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Properties(pub HashMap<String, Value>);

impl Properties {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn from_json(value: Value) -> std::result::Result<Self, serde_json::Error> {
        let map = if let Value::Object(map) = value {
            map.into_iter()
                .map(|(k, v)| (k, v))
                .collect()
        } else {
            HashMap::new()
        };
        Ok(Self(map))
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn insert(&mut self, key: String, value: Value) {
        self.0.insert(key, value);
    }
}

/// A node in the temporal knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub entity_type: EntityType,
    pub label: String,
    pub properties: Properties,
    pub valid_time: TemporalRange,
    pub transaction_time: TemporalRange,
}

impl Node {
    pub fn id(&self) -> &NodeId {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn properties(&self) -> &Properties {
        &self.properties
    }
}

/// An edge in the temporal knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub source_id: NodeId,
    pub target_id: NodeId,
    pub label: String,
    pub properties: Properties,
    pub valid_time: TemporalRange,
    pub transaction_time: TemporalRange,
}

impl Edge {
    pub fn id(&self) -> &EdgeId {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn properties(&self) -> &Properties {
        &self.properties
    }

    pub fn out_v(&self) -> &NodeId {
        &self.source_id
    }

    pub fn in_v(&self) -> &NodeId {
        &self.target_id
    }
}

/// Represents a temporal operation type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemporalOperation {
    /// Query at a specific point in time
    At(DateTime<Utc>),
    /// Query between two points in time
    Between(DateTime<Utc>, DateTime<Utc>),
    /// Query the evolution over a time range
    Evolution(TemporalRange),
    /// Query the latest state
    Latest,
}

/// Represents a temporal query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalQueryResult<T> {
    /// The query result data
    pub data: T,
    /// The timestamp at which this result is valid
    pub timestamp: Timestamp,
    /// The version identifier
    pub version_id: Uuid,
}

impl<T> TemporalQueryResult<T> {
    /// Create a new temporal query result
    pub fn new(data: T, timestamp: Timestamp, version_id: Uuid) -> Self {
        Self {
            data,
            timestamp,
            version_id,
        }
    }

    /// Get a reference to the data
    pub fn data(&self) -> &T {
        &self.data
    }

    /// Get a reference to the timestamp
    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }

    /// Get a reference to the version ID
    pub fn version_id(&self) -> &Uuid {
        &self.version_id
    }
}

impl<T> std::ops::Deref for TemporalQueryResult<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> AsRef<T> for TemporalQueryResult<T> {
    fn as_ref(&self) -> &T {
        &self.data
    }
}

/// Represents metadata about a temporal entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalMetadata {
    /// Entity ID
    pub entity_id: EntityId,
    /// Creation time
    pub created_at: Timestamp,
    /// Last modification time
    pub modified_at: Timestamp,
    /// Version identifier
    pub version_id: Uuid,
    /// Valid time range
    pub valid_time: TemporalRange,
}

impl TemporalMetadata {
    /// Create new temporal metadata
    pub fn new(entity_id: EntityId, valid_time: TemporalRange) -> Self {
        let now = Timestamp::now();
        Self {
            entity_id,
            created_at: now,
            modified_at: now,
            version_id: Uuid::new_v4(),
            valid_time,
        }
    }

    /// Update the modification time
    pub fn touch(&mut self) {
        self.modified_at = Timestamp::now();
        self.version_id = Uuid::new_v4();
    }
}

/// Gremlin ID type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GID(pub String);

impl GID {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Time range type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub transaction_time: (DateTime<Utc>, DateTime<Utc>),
    pub valid_time: (DateTime<Utc>, DateTime<Utc>),
}

impl TimeRange {
    pub fn new(transaction_time: (DateTime<Utc>, DateTime<Utc>), valid_time: (DateTime<Utc>, DateTime<Utc>)) -> Self {
        Self {
            transaction_time,
            valid_time,
        }
    }
}

/// Wrapper type for Node to avoid orphan rule violations
#[derive(Debug, Clone)]
pub struct GNode(pub Node);

/// Wrapper type for Edge to avoid orphan rule violations
#[derive(Debug, Clone)]
pub struct GEdge(pub Edge);

/// Local wrapper type for GResultSet to avoid orphan rule violations
#[derive(Debug)]
pub struct LocalResultSet(pub Vec<GValue>);

impl LocalResultSet {
    pub fn new(result_set: GResultSet) -> Self {
        // Handle each result in the GResultSet, collecting only successful values
        let results: Vec<GValue> = result_set.into_iter()
            .filter_map(|res| res.ok())
            .collect();
        LocalResultSet(results)
    }
}

/// Trait for converting from our local result set type
pub trait FromLocalResultSet: Sized {
    fn from_local_result_set(result_set: LocalResultSet) -> Result<Self>;
}

// Helper function to fix properties conversion
fn convert_property_value(value: GValue) -> serde_json::Value {
    match value {
        GValue::String(s) => serde_json::Value::String(s),
        GValue::Int32(i) => serde_json::Value::Number(serde_json::Number::from(i as i64)),
        GValue::Int64(i) => serde_json::Value::Number(serde_json::Number::from(i)),
        GValue::Float(f) => serde_json::Number::from_f64(f64::from(f))
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        GValue::Double(d) => serde_json::Number::from_f64(d)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        GValue::List(l) => serde_json::Value::Array(
            l.into_iter().map(convert_property_value).collect()
        ),
        GValue::Map(m) => {
            let mut map = serde_json::Map::new();
            for (k, v) in m {
                let key = match k {
                    gremlin_client::GKey::String(s) => s,
                    _ => format!("{:?}", k), // Handle other key types
                };
                map.insert(key, convert_property_value(v));
            }
            serde_json::Value::Object(map)
        },
        _ => serde_json::Value::Null,
    }
}

// Implement for Node
impl FromLocalResultSet for Node {
    fn from_local_result_set(result_set: LocalResultSet) -> Result<Self> {
        let mut results = result_set.0;
        if results.is_empty() {
            return Err(Error::NotFound("Node not found".to_string()));
        }
        
        let vertex = results.remove(0);
        match vertex {
            GValue::Vertex(v) => {
                let now = Utc::now();
                let id = vertex_id_to_string(&v);
                
                let label = v.label().to_string();
                
                // Process properties
                let properties = {
                    let mut props = HashMap::new();
                    for (key, value) in v.iter() {
                        if !value.is_empty() {
                            // Just take the first property value for each key
                            props.insert(key.clone(), convert_property_value(value[0].value().clone()));
                        }
                    }
                    Properties(props)
                };

                Ok(Node {
                    id: NodeId(Uuid::from_str(&id)?),
                    entity_type: EntityType::from_str(&label)?,
                    label,
                    properties,
                    valid_time: TemporalRange {
                        start: Some(Timestamp(now)),
                        end: None,
                    },
                    transaction_time: TemporalRange {
                        start: Some(Timestamp(now)),
                        end: None,
                    },
                })
            },
            _ => Err(Error::Neptune("Expected vertex result".to_string())),
        }
    }
}

// Implement for Edge
impl FromLocalResultSet for Edge {
    fn from_local_result_set(result_set: LocalResultSet) -> Result<Self> {
        let mut results = result_set.0;
        if results.is_empty() {
            return Err(Error::NotFound("Edge not found".to_string()));
        }
        
        let edge = results.remove(0);
        match edge {
            GValue::Edge(e) => {
                let now = Utc::now();
                let id = edge_id_to_string(&e);
                
                let out_v_str = vertex_id_to_string(e.out_v());
                
                let in_v_str = vertex_id_to_string(e.in_v());
                
                // Process properties
                let properties = {
                    let mut props = HashMap::new();
                    for (key, value) in e.iter() {
                        props.insert(key.clone(), convert_property_value(value.value().clone()));
                    }
                    Properties(props)
                };

                Ok(Edge {
                    id: EdgeId(Uuid::from_str(&id)?),
                    source_id: NodeId(Uuid::from_str(&out_v_str)?),
                    target_id: NodeId(Uuid::from_str(&in_v_str)?),
                    label: e.label().to_string(),
                    properties,
                    valid_time: TemporalRange {
                        start: Some(Timestamp(now)),
                        end: None,
                    },
                    transaction_time: TemporalRange {
                        start: Some(Timestamp(now)),
                        end: None,
                    },
                })
            },
            _ => Err(Error::Neptune("Expected edge result".to_string())),
        }
    }
}

// Implement for Vec<Node>
impl FromLocalResultSet for Vec<Node> {
    fn from_local_result_set(result_set: LocalResultSet) -> Result<Self> {
        result_set.0.into_iter()
            .map(|value| match value {
                GValue::Vertex(v) => {
                    let now = Utc::now();
                    let id = vertex_id_to_string(&v);
                    let label = v.label().to_string();
                    let properties = {
                        let mut props = HashMap::new();
                        for (key, value) in v.iter() {
                            if !value.is_empty() {
                                // Just take the first property value for each key
                                props.insert(key.clone(), convert_property_value(value[0].value().clone()));
                            }
                        }
                        Properties(props)
                    };

                    Ok(Node {
                        id: NodeId(Uuid::from_str(&id)?),
                        entity_type: EntityType::from_str(&label)?,
                        label,
                        properties,
                        valid_time: TemporalRange {
                            start: Some(Timestamp(now)),
                            end: None,
                        },
                        transaction_time: TemporalRange {
                            start: Some(Timestamp(now)),
                            end: None,
                        },
                    })
                },
                _ => Err(Error::Neptune("Expected vertex result".to_string())),
            })
            .collect()
    }
}

// Implement for Vec<Edge>
impl FromLocalResultSet for Vec<Edge> {
    fn from_local_result_set(result_set: LocalResultSet) -> Result<Self> {
        result_set.0.into_iter()
            .map(|value| match value {
                GValue::Edge(e) => {
                    let now = Utc::now();
                    let id = edge_id_to_string(&e);
                    let properties = {
                        let mut props = HashMap::new();
                        for (key, value) in e.iter() {
                            props.insert(key.clone(), convert_property_value(value.value().clone()));
                        }
                        Properties(props)
                    };

                    Ok(Edge {
                        id: EdgeId(Uuid::from_str(&id)?),
                        source_id: NodeId(Uuid::from_str(&vertex_id_to_string(e.out_v()))?),
                        target_id: NodeId(Uuid::from_str(&vertex_id_to_string(e.in_v()))?),
                        label: e.label().to_string(),
                        properties,
                        valid_time: TemporalRange {
                            start: Some(Timestamp(now)),
                            end: None,
                        },
                        transaction_time: TemporalRange {
                            start: Some(Timestamp(now)),
                            end: None,
                        },
                    })
                },
                _ => Err(Error::Neptune("Expected edge result".to_string())),
            })
            .collect()
    }
}

// Implement for Option<Node>
impl FromLocalResultSet for Option<Node> {
    fn from_local_result_set(result_set: LocalResultSet) -> Result<Self> {
        let mut results = result_set.0;
        if results.is_empty() {
            return Ok(None);
        }
        
        let vertex = results.remove(0);
        match vertex {
            GValue::Vertex(v) => {
                let now = Utc::now();
                let id = vertex_id_to_string(&v);
                let label = v.label().to_string();
                let properties = {
                    let mut props = HashMap::new();
                    for (key, value) in v.iter() {
                        if !value.is_empty() {
                            // Just take the first property value for each key
                            props.insert(key.clone(), convert_property_value(value[0].value().clone()));
                        }
                    }
                    Properties(props)
                };

                Ok(Some(Node {
                    id: NodeId(Uuid::from_str(&id)?),
                    entity_type: EntityType::from_str(&label)?,
                    label,
                    properties,
                    valid_time: TemporalRange {
                        start: Some(Timestamp(now)),
                        end: None,
                    },
                    transaction_time: TemporalRange {
                        start: Some(Timestamp(now)),
                        end: None,
                    },
                }))
            },
            _ => Err(Error::Neptune("Expected vertex result".to_string())),
        }
    }
}

// Implement for unit type
impl FromLocalResultSet for () {
    fn from_local_result_set(_: LocalResultSet) -> Result<Self> {
        Ok(())
    }
}

impl ToGValue for Node {
    fn to_gvalue(&self) -> GValue {
        let mut map = HashMap::new();
        map.insert("id".to_string(), self.id.to_gvalue());
        map.insert("entity_type".to_string(), self.entity_type.to_gvalue());
        map.insert("label".to_string(), GValue::String(self.label.clone()));
        map.insert("properties".to_string(), self.properties.to_gvalue());
        map.insert("valid_time".to_string(), self.valid_time.to_gvalue());
        map.insert("transaction_time".to_string(), self.transaction_time.to_gvalue());
        GValue::Map(map.into())
    }
}

impl ToGValue for Edge {
    fn to_gvalue(&self) -> GValue {
        let mut map = HashMap::new();
        map.insert("id".to_string(), self.id.to_gvalue());
        map.insert("source_id".to_string(), self.source_id.to_gvalue());
        map.insert("target_id".to_string(), self.target_id.to_gvalue());
        map.insert("label".to_string(), GValue::String(self.label.clone()));
        map.insert("properties".to_string(), self.properties.to_gvalue());
        map.insert("valid_time".to_string(), self.valid_time.to_gvalue());
        map.insert("transaction_time".to_string(), self.transaction_time.to_gvalue());
        GValue::Map(map.into())
    }
}

impl GNode {
    /// Convert to Node
    pub fn into_node(self) -> Node {
        self.0
    }
}

impl GEdge {
    /// Convert to Edge
    pub fn into_edge(self) -> Edge {
        self.0
    }
}

// Add wrapper types for external types to avoid orphan rule violations
/// Wrapper for GValue for ToGValue implementation
#[derive(Debug, Clone)]
pub struct LocalGValue(pub GValue);

/// Wrapper for serde_json::Value for conversion implementation
#[derive(Debug, Clone)]
pub struct LocalJsonValue(pub serde_json::Value);

impl LocalGValue {
    /// Convert to serde_json::Value
    pub fn into_json(self) -> serde_json::Value {
        match self.0 {
            GValue::String(s) => serde_json::Value::String(s),
            GValue::Int32(i) => serde_json::Value::Number(i.into()),
            GValue::Int64(i) => serde_json::Value::Number(i.into()),
            GValue::Float(f) => serde_json::Number::from_f64(f64::from(f))
                .map_or(serde_json::Value::Null, serde_json::Value::Number),
            GValue::Double(d) => serde_json::Number::from_f64(d)
                .map_or(serde_json::Value::Null, serde_json::Value::Number),
            GValue::List(l) => serde_json::Value::Array(
                l.into_iter().map(|v| LocalGValue(v).into_json()).collect()
            ),
            GValue::Map(m) => {
                let mut map = serde_json::Map::new();
                for (k, v) in m {
                    let key = match k {
                        gremlin_client::GKey::String(s) => s,
                        _ => format!("{:?}", k), // Handle other key types as strings
                    };
                    map.insert(key, LocalGValue(v).into_json());
                }
                serde_json::Value::Object(map)
            },
            GValue::Null => serde_json::Value::Null,
            _ => serde_json::Value::Null,
        }
    }
}

impl ToGValue for LocalGValue {
    fn to_gvalue(&self) -> GValue {
        self.0.clone()
    }
}

/// Convert GValue to serde_json::Value
fn convert_gvalue_to_json(value: &GValue) -> serde_json::Value {
    match value {
        GValue::String(s) => serde_json::Value::String(s.clone()),
        GValue::Int32(i) => serde_json::Value::Number(serde_json::Number::from(*i as i64)),
        GValue::Int64(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
        GValue::Float(f) => serde_json::Number::from_f64(f64::from(*f))
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        GValue::Double(d) => serde_json::Number::from_f64(*d)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        GValue::List(l) => serde_json::Value::Array(
            l.iter().map(|v| convert_gvalue_to_json(v)).collect()
        ),
        GValue::Map(m) => {
            let mut map = serde_json::Map::new();
            for (k, v) in m.iter() {
                let key = match k {
                    gremlin_client::GKey::String(s) => s.clone(),
                    _ => format!("{:?}", k), // Handle other key types as strings
                };
                map.insert(key, convert_gvalue_to_json(v));
            }
            serde_json::Value::Object(map)
        },
        _ => serde_json::Value::Null,
    }
}

impl FromLocalResultSet for NodeId {
    fn from_local_result_set(result_set: LocalResultSet) -> Result<Self> {
        if result_set.0.is_empty() {
            return Err(Error::NotFound("No node ID found in result set".to_string()));
        }

        let value = &result_set.0[0];
        match value {
            GValue::String(s) => {
                let id = Uuid::parse_str(s)
                    .map_err(|e| Error::InvalidId(format!("Failed to parse UUID: {}", e)))?;
                Ok(NodeId(id))
            }
            _ => Err(Error::InvalidId(format!("Expected string value for node ID, got {:?}", value))),
        }
    }
}

impl FromLocalResultSet for EdgeId {
    fn from_local_result_set(result_set: LocalResultSet) -> Result<Self> {
        if result_set.0.is_empty() {
            return Err(Error::NotFound("No edge ID found in result set".to_string()));
        }

        let value = &result_set.0[0];
        match value {
            GValue::String(s) => {
                let id = Uuid::parse_str(s)
                    .map_err(|e| Error::InvalidId(format!("Failed to parse UUID: {}", e)))?;
                Ok(EdgeId(id))
            }
            _ => Err(Error::InvalidId(format!("Expected string value for edge ID, got {:?}", value))),
        }
    }
}

/// Convert a GID to a String
pub fn gid_to_string(gid: &gremlin_client::GID) -> String {
    match gid {
        gremlin_client::GID::String(s) => s.clone(),
        gremlin_client::GID::Int32(i) => i.to_string(),
        gremlin_client::GID::Int64(i) => i.to_string(),
        _ => format!("{:?}", gid), // Fallback to debug representation
    }
}

/// Extract ID from a Vertex
pub fn vertex_id_to_string(vertex: &gremlin_client::Vertex) -> String {
    gid_to_string(vertex.id())
}

/// Extract ID from an Edge
pub fn edge_id_to_string(edge: &gremlin_client::Edge) -> String {
    gid_to_string(edge.id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn test_entity_id() {
        let id1 = EntityId::new(EntityType::Node, "test1");
        let id2 = EntityId::new(EntityType::Node, "test2");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_timestamp() {
        let now = Utc::now();
        let ts = Timestamp::from_datetime(now);
        assert_eq!(ts.0, now);
    }

    #[test]
    fn test_temporal_range() {
        let now = Utc::now();
        let later = now + chrono::Duration::hours(1);
        
        let range = TemporalRange::new(Some(Timestamp(now)), Some(Timestamp(later)));
        
        assert!(range.contains(&now));
        assert!(range.contains(&later));
        assert!(!range.contains(&(later + chrono::Duration::hours(1))));
    }

    #[test]
    fn test_temporal_metadata() {
        let entity_id = EntityId::new(EntityType::Node, "test");
        let range = TemporalRange::from_now();
        
        let mut metadata = TemporalMetadata::new(entity_id.clone(), range);
        let original_version = metadata.version_id;
        
        metadata.touch();
        
        assert_eq!(metadata.entity_id, entity_id);
        assert_ne!(metadata.version_id, original_version);
        assert!(metadata.modified_at.0 >= metadata.created_at.0);
    }

    #[test]
    fn test_node_serialization() {
        let node = Node {
            id: NodeId(Uuid::new_v4()),
            entity_type: EntityType::Node,
            label: "test".to_string(),
            properties: Properties(HashMap::new()),
            valid_time: TemporalRange {
                start: Some(Timestamp(Utc::now())),
                end: None,
            },
            transaction_time: TemporalRange {
                start: Some(Timestamp(Utc::now())),
                end: None,
            },
        };
        
        let json = serde_json::to_string(&node).unwrap();
        let deserialized: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node.id, deserialized.id);
    }
} 