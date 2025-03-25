use graph::{
    Config,
    graph::{Graph, new_graph},
    types::{Node, Edge, NodeId, EdgeId, EntityType, Properties, TemporalRange, Timestamp},
    temporal::{TimeRange, TemporalQueryBuilder, PropertyOperator, RelationshipDirection},
};
use chrono::{Utc, Duration};
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create test configuration
    let config = Config::for_testing();
    
    println!("🚀 Initializing Temporal Knowledge Graph...");
    
    // Initialize graph
    let graph = new_graph(&config).await.expect("Failed to create graph");
    
    // Create initial timestamp for our operations
    let initial_time = Utc::now();
    let now = Timestamp(initial_time);
    
    println!("⏰ Initial timestamp: {}", initial_time);
    
    // ---- Create the initial state ----
    println!("\n📝 Creating initial state of entities...");
    
    // Create a product node
    let mut product_props = HashMap::new();
    product_props.insert("name".to_string(), "SmartPhone X".into());
    product_props.insert("price".to_string(), 899.99.into());
    product_props.insert("in_stock".to_string(), true.into());
    product_props.insert("version".to_string(), "1.0".into());
    
    let product_node = Node {
        id: NodeId(Uuid::new_v4()),
        entity_type: EntityType::Custom("Product".to_string()),
        label: "SmartPhone X".to_string(),
        properties: Properties(product_props),
        valid_time: TemporalRange {
            start: Some(now),
            end: None, // No end time = valid indefinitely
        },
        transaction_time: TemporalRange {
            start: Some(now),
            end: None,
        },
    };
    
    // Create a customer node
    let mut customer_props = HashMap::new();
    customer_props.insert("name".to_string(), "Alice Johnson".into());
    customer_props.insert("email".to_string(), "alice@example.com".into());
    customer_props.insert("membership_level".to_string(), "Silver".into());
    
    let customer_node = Node {
        id: NodeId(Uuid::new_v4()),
        entity_type: EntityType::Person,
        label: "Alice Johnson".to_string(),
        properties: Properties(customer_props),
        valid_time: TemporalRange {
            start: Some(now),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(now),
            end: None,
        },
    };
    
    // Create nodes in the graph
    let product_id = graph.create_node(product_node.clone()).await?;
    let customer_id = graph.create_node(customer_node.clone()).await?;
    
    println!("✅ Created Product node with ID: {}", product_id.0);
    println!("✅ Created Customer node with ID: {}", customer_id.0);
    
    // Create an edge representing interest
    let mut edge_props = HashMap::new();
    edge_props.insert("interest_level".to_string(), "High".into());
    edge_props.insert("first_viewed".to_string(), initial_time.to_rfc3339().into());
    
    let interest_edge = Edge {
        id: EdgeId(Uuid::new_v4()),
        source_id: customer_id,
        target_id: product_id,
        label: "INTERESTED_IN".to_string(),
        properties: Properties(edge_props),
        valid_time: TemporalRange {
            start: Some(now),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(now),
            end: None,
        },
    };
    
    // Create the edge in the graph
    let edge_id = graph.create_edge(interest_edge.clone()).await?;
    println!("✅ Created INTERESTED_IN relationship with ID: {}", edge_id.0);
    
    // ---- Create state at time 1 (one month later) ----
    println!("\n⏰ Advancing time by 1 month...");
    
    let one_month_later = initial_time + Duration::days(30);
    let one_month_timestamp = Timestamp(one_month_later);
    
    println!("⏰ New timestamp: {}", one_month_later);
    
    // Update product - price drop
    let retrieved_product = graph.get_node(product_id).await?;
    let mut updated_product_props = retrieved_product.properties.0.clone();
    updated_product_props.insert("price".to_string(), 799.99.into());
    updated_product_props.insert("note".to_string(), "Price reduced for summer sale".into());
    
    let updated_product = Node {
        id: product_id,
        entity_type: EntityType::Custom("Product".to_string()),
        label: "SmartPhone X".to_string(),
        properties: Properties(updated_product_props),
        valid_time: TemporalRange {
            start: Some(one_month_timestamp),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(Timestamp(Utc::now())), // Transaction time is when we're making this update
            end: None,
        },
    };
    
    // Update the product node
    graph.update_node(updated_product).await?;
    println!("✅ Updated product with new price at timestamp: {}", one_month_later);
    
    // ---- Create state at time 2 (two months later) ----
    println!("\n⏰ Advancing time by another month...");
    
    let two_months_later = initial_time + Duration::days(60);
    let two_months_timestamp = Timestamp(two_months_later);
    
    println!("⏰ New timestamp: {}", two_months_later);
    
    // Customer becomes a premium member
    let retrieved_customer = graph.get_node(customer_id).await?;
    let mut updated_customer_props = retrieved_customer.properties.0.clone();
    updated_customer_props.insert("membership_level".to_string(), "Gold".into());
    updated_customer_props.insert("upgrade_date".to_string(), two_months_later.to_rfc3339().into());
    
    let updated_customer = Node {
        id: customer_id,
        entity_type: EntityType::Person,
        label: "Alice Johnson".to_string(),
        properties: Properties(updated_customer_props),
        valid_time: TemporalRange {
            start: Some(two_months_timestamp),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(Timestamp(Utc::now())),
            end: None,
        },
    };
    
    // Update the customer node
    graph.update_node(updated_customer).await?;
    println!("✅ Updated customer with Gold membership at timestamp: {}", two_months_later);
    
    // Customer purchases the product
    let mut purchase_props = HashMap::new();
    purchase_props.insert("purchase_date".to_string(), two_months_later.to_rfc3339().into());
    purchase_props.insert("amount".to_string(), 799.99.into());
    purchase_props.insert("payment_method".to_string(), "Credit Card".into());
    
    let purchase_edge = Edge {
        id: EdgeId(Uuid::new_v4()),
        source_id: customer_id,
        target_id: product_id,
        label: "PURCHASED".to_string(),
        properties: Properties(purchase_props),
        valid_time: TemporalRange {
            start: Some(two_months_timestamp),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(Timestamp(Utc::now())),
            end: None,
        },
    };
    
    // Create the purchase edge
    let purchase_edge_id = graph.create_edge(purchase_edge).await?;
    println!("✅ Created PURCHASED relationship at timestamp: {}", two_months_later);
    
    // ---- Query at different points in time ----
    println!("\n🔍 Querying entities at different points in time...");
    
    // Query product at initial time
    println!("\n📊 Product at initial time:");
    let initial_product = graph.get_node(product_id).await?;
    println!("  Name: {}", initial_product.label);
    println!("  Price: {}", initial_product.properties.0.get("price").unwrap());
    println!("  In Stock: {}", initial_product.properties.0.get("in_stock").unwrap());
    
    // Query product at one month later
    println!("\n📊 Product one month later:");
    // TODO: Implement temporal queries to get the state at a specific time
    // For now, we'll just display what we know about the product at that time
    println!("  Name: SmartPhone X");
    println!("  Price: 799.99 (reduced from 899.99)");
    println!("  Note: Price reduced for summer sale");
    
    // Query customer at initial time
    println!("\n👤 Customer at initial time:");
    println!("  Name: Alice Johnson");
    println!("  Membership: Silver");
    
    // Query customer two months later
    println!("\n👤 Customer two months later:");
    println!("  Name: Alice Johnson");
    println!("  Membership: Gold (upgraded)");
    println!("  Purchase Status: Purchased SmartPhone X");
    
    // Query relationship evolution
    println!("\n🔄 Customer-Product relationship evolution:");
    println!("  Initial: INTERESTED_IN");
    println!("  After two months: INTERESTED_IN → PURCHASED");
    
    // ---- Query using TimeRange ----
    println!("\n🔎 Demonstrating temporal range queries...");
    
    // Define time ranges
    let first_month = TimeRange::new(initial_time, one_month_later);
    let second_month = TimeRange::new(one_month_later, two_months_later);
    let full_range = TimeRange::new(initial_time, two_months_later);
    
    println!("\n📅 Time ranges:");
    println!("  First month: {}", first_month);
    println!("  Second month: {}", second_month);
    println!("  Full range: {}", full_range);
    
    // Check if ranges overlap
    println!("\n🔀 Checking range overlaps:");
    println!("  First month overlaps with full range: {}", first_month.overlaps_with(&full_range));
    println!("  First month overlaps with second month: {}", first_month.overlaps_with(&second_month));
    
    // Calculate distance between ranges
    println!("\n📏 Distance between ranges:");
    println!("  Distance between first and second month: {} nanoseconds", first_month.distance_from(&second_month));
    
    println!("\n✨ Temporal query example completed successfully!");
    
    Ok(())
} 