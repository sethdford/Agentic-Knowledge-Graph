# Temporal Knowledge Graph Examples

This directory contains examples that demonstrate how to use the Temporal Knowledge Graph API. These examples show various features and usage patterns to help you get started with your own applications.

## Prerequisites

Before running these examples, make sure you have:

1. Rust and Cargo installed (see [rustup.rs](https://rustup.rs) for installation)
2. Dependencies installed with `cargo build`
3. For API client examples, the server running on localhost:3000 (start with `cargo run`)

## Available Examples

### 1. Basic Operations (`basic_operations.rs`)

Demonstrates core graph operations including:
- Creating nodes and edges
- Retrieving entities
- Querying connections
- Basic temporal features

**Run with:**
```bash
cargo run --example basic_operations
```

This example creates a person and organization node, establishes a relationship between them, and then demonstrates querying the graph. It also shows how to create new versions of entities that become valid at future times.

### 2. API Client (`api_client.rs`)

Shows how to interact with the HTTP API from a client application:
- Making HTTP requests to the API endpoints
- Creating, retrieving, and updating graph entities
- Working with JSON payloads and responses
- Handling errors

**Run with:**
```bash
# First start the server in another terminal
cargo run

# Then run the client example
cargo run --example api_client
```

This example creates a simple client for interacting with the API, demonstrating how external applications can leverage the Temporal Knowledge Graph.

### 3. Temporal Query (`temporal_query.rs`)

Focuses on the temporal aspects of the knowledge graph:
- Creating entities with specific valid times
- Tracking the evolution of entities over time
- Working with time ranges
- Performing queries at different points in time

**Run with:**
```bash
cargo run --example temporal_query
```

This example creates a product and customer entity, then updates them at different points in time to demonstrate how the graph captures the history and evolution of entities.

### 4. Hybrid Vector+Graph Operations (`hybrid_operations.rs`)

Demonstrates the hybrid architecture that combines graph traversal with vector similarity:
- Creating nodes and edges with vector embeddings
- Performing vector similarity searches
- Executing hybrid queries that combine graph traversal and vector similarity
- Using fusion strategies to combine results
- Temporal queries with vector similarity

**Run with:**
```bash
cargo run --example hybrid_operations
```

This example creates a small knowledge graph with vector embeddings attached to nodes and edges, then demonstrates how to perform vector similarity searches, hybrid queries, and temporal queries with vector similarity, showcasing the power of the hybrid approach.

### 5. Memory Operations (New)

Demonstrates how to work with the vector memory system:
- Creating and storing memory entries
- Retrieving entries by ID
- Searching for similar entries using vector similarity

**Run with:**
```bash
cargo run --example memory_operations
```

### 6. Hybrid Operations (New)

Shows how to combine graph database and vector memory operations:
- Creating nodes in the graph database
- Storing the same entities with vector embeddings
- Demonstrating hybrid queries using both graph traversal and vector similarity

**Run with:**
```bash
cargo run --example hybrid_operations
```

## End-to-End Tests

In addition to these examples, you can find end-to-end tests in the `tests/` directory:

- `api_e2e_test.rs`: Tests the API endpoints with a simulated client
- `graph_integration_test.rs`: Tests the underlying graph operations

**Run tests with:**
```bash
# Run normal tests
cargo test

# Run integration tests (including API tests)
cargo test -- --ignored
```

## Adding Your Own Examples

Feel free to create your own examples based on these templates. Here's a simple structure to follow:

1. Create a new file in the `examples/` directory
2. Import the necessary modules from the `graph` crate
3. Write a main function that demonstrates your use case
4. Add your example to this README.md

## Using the Examples as Reference

These examples are designed to be used as reference implementations. You can:

1. Copy and adapt code snippets for your own applications
2. Use them to understand the API design and capabilities
3. Learn how to structure temporal data in your applications
4. Explore advanced features like temporal querying and graph traversal

## Common Patterns

Throughout these examples, you'll notice some common patterns:

1. **Entity Creation**: Nodes and edges are created with properties and temporal ranges
2. **Temporal Versioning**: Entities are updated with new valid times to create temporal versions
3. **Graph Querying**: Various methods are used to traverse and query the graph
4. **Error Handling**: Examples demonstrate proper error handling patterns
5. **Vector Similarity**: Hybrid operations use vector embeddings for enhanced retrieval capabilities
6. **Fusion Strategies**: Different strategies for combining graph and vector results are demonstrated

## Need Help?

If you have questions or need assistance:
1. Check the documentation in the codebase
2. Refer to the API documentation available at `/swagger-ui` when the server is running
3. Look at the test files for more usage examples 