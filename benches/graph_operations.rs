use chrono::Utc;
use uuid::Uuid;
use std::sync::Arc;
use criterion::{Criterion, criterion_group, criterion_main, BatchSize};

use graph::{
    temporal::{Temporal, DynamoDBTemporal, TemporalQueryBuilder},
    types::{Node, EntityType, Properties, NodeId, EntityId, TemporalRange, Timestamp},
    aws::dynamodb::DynamoDBClient,
};

use aws_sdk_dynamodb::Client as DynamoClient;

fn create_test_node() -> (EntityId, Node, TemporalRange) {
    let now = Utc::now();
    let valid_time = TemporalRange {
        start: Some(Timestamp(now)),
        end: Some(Timestamp(now + chrono::Duration::days(1))),
    };

    let node = Node {
        id: NodeId(Uuid::new_v4()),
        entity_type: EntityType::Person,
        properties: Properties::new(),
        label: "Test Node".to_string(),
        valid_time: valid_time.clone(),
        transaction_time: valid_time.clone(),
    };
    let entity_id = EntityId {
        entity_type: EntityType::Node,
        id: node.id.0.to_string(),
    };
    (entity_id, node, valid_time)
}

fn bench_graph_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let graph = rt.block_on(async {
        let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region("us-east-1")
            .load()
            .await;
        let client = Arc::new(DynamoClient::new(&aws_config));
        Arc::new(DynamoDBTemporal::<Node, DynamoClient>::new(client, "bench_graph".to_string()))
    });
    
    // Benchmark: Store Operation
    c.bench_function("store_node", |b| {
        b.iter_batched_ref(
            create_test_node,
            |(entity_id, node, valid_time)| {
                rt.block_on(async {
                    Temporal::store(&*graph, entity_id.clone(), node.clone(), valid_time.clone()).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Benchmark: Query At Point in Time
    c.bench_function("query_at", |b| {
        b.iter_batched_ref(
            || {
                let (entity_id, _, _) = create_test_node();
                let now = Utc::now();
                (entity_id, now)
            },
            |(entity_id, now)| {
                rt.block_on(async {
                    graph.query_at(&entity_id, *now).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Benchmark: Query Between Time Range
    c.bench_function("query_between", |b| {
        b.iter_batched_ref(
            || {
                let (entity_id, _, _) = create_test_node();
                let now = Utc::now();
                let start = now - chrono::Duration::hours(1);
                let end = now + chrono::Duration::hours(1);
                (entity_id, start, end)
            },
            |(entity_id, start, end)| {
                rt.block_on(async {
                    graph.query_between(&entity_id, *start, *end).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Benchmark: Query Evolution
    c.bench_function("query_evolution", |b| {
        b.iter_batched_ref(
            || {
                let (entity_id, _, _) = create_test_node();
                let now = Utc::now();
                let range = TemporalRange {
                    start: Some(Timestamp(now - chrono::Duration::hours(24))),
                    end: Some(Timestamp(now)),
                };
                (entity_id, range)
            },
            |(entity_id, range)| {
                rt.block_on(async {
                    graph.query_evolution(&entity_id, &range).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Benchmark: Query Latest
    c.bench_function("query_latest", |b| {
        b.iter_batched_ref(
            || {
                let (entity_id, _, _) = create_test_node();
                entity_id
            },
            |entity_id| {
                rt.block_on(async {
                    graph.query_latest(&entity_id).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Benchmark: Validate Consistency
    c.bench_function("validate_consistency", |b| {
        b.iter_batched_ref(
            || {},
            |_| {
                rt.block_on(async {
                    graph.validate_consistency().await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_graph_operations);
criterion_main!(benches);