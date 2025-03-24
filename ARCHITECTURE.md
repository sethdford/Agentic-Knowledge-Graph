# Zep Temporal Knowledge Graph Architecture

## Overview

Zep is a temporal knowledge graph system that provides dynamic memory capabilities for AI agents. The system is built on AWS managed services and implements a bi-temporal model for tracking both chronological events and data ingestion.

This implementation is based on the research presented in ["Temporal Knowledge Graphs for AI Agent Memory"](https://arxiv.org/html/2501.13956v1), which introduces several key innovations:

1. **Bi-Temporal Modeling**: Tracking both valid time (when events occurred) and transaction time (when data was recorded)
2. **Hierarchical Memory**: Organization of knowledge into episodic, semantic, and working memory
3. **Temporal Consistency**: Maintaining data integrity across time dimensions
4. **Efficient Querying**: Optimized retrieval of temporal knowledge

Our approach builds upon and extends existing solutions in the space:

- [Graphiti](https://github.com/getzep/graphiti): We leverage similar graph database patterns but extend them with temporal capabilities
- [Zep Memory Server](https://www.getzep.com/): We incorporate comparable memory management features while focusing on temporal aspects

The architecture is specifically designed to address several key challenges in AI agent memory:

- **Temporal Reasoning**: Enabling agents to understand and reason about time-based relationships
- **Memory Consistency**: Ensuring reliable and accurate historical knowledge
- **Scalable Storage**: Managing growing knowledge bases efficiently
- **Fast Retrieval**: Providing quick access to relevant temporal context
- **Real-time Updates**: Supporting dynamic knowledge updates while maintaining consistency

https://arxiv.org/html/2501.13956v1 
https://github.com/getzep/graphiti
https://www.getzep.com/

## Core Components

### 1. Graph Engine (Graphiti)

#### Structure
- **Episode Subgraph**: Stores raw input data (messages, text, JSON)
- **Semantic Entity Subgraph**: Contains extracted entities and relationships
- **Community Subgraph**: Manages higher-level groupings and patterns

#### Temporal Model
- **T Timeline**: Chronological ordering of events
- **T' Timeline**: Transactional ordering of data ingestion
- **Bi-Temporal Tracking**: Maintains both valid time and transaction time

### 2. Memory System

#### Vector Storage
- **OpenSearch Integration**: Manages vector embeddings
- **Similarity Search**: Fast retrieval of related content
- **Caching Layer**: Optimizes frequent queries

#### Memory Types
- **Episodic Memory**: Raw conversation and event data
- **Semantic Memory**: Processed knowledge and relationships
- **Working Memory**: Active context and recent interactions

### 3. RAG (Retrieval-Augmented Generation)

#### Components
- **Entity Extraction**: Identifies key concepts and relationships
- **Context Retrieval**: Fetches relevant historical information
- **Knowledge Integration**: Combines retrieved context with current state

#### Features
- **Streaming Support**: Handles large text inputs
- **Context Window Management**: Optimizes context usage
- **Confidence Scoring**: Ranks retrieval relevance

## AWS Infrastructure

### Core Services
```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Amazon Neptune │     │    OpenSearch   │     │    DynamoDB     │
│  (Graph Store)  │     │(Vector Storage) │     │   (Metadata)    │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                        │
         └───────────────┬──────────────────────────────┘
                        │
                ┌───────┴───────┐
                │   API Layer   │
                └───────┬───────┘
                       │
         ┌────────────┴────────────┐
         │     Application         │
         └────────────┬────────────┘
                     │
    ┌────────────────┴───────────────┐
    │        AWS Infrastructure      │
    └────────────────────────────────┘
```

### Supporting Services
- **S3**: Raw data storage
- **SQS**: Asynchronous processing
- **CloudWatch**: Monitoring and metrics
- **X-Ray**: Distributed tracing
- **IAM**: Access control

## API Layer

### REST Endpoints
- `/store`: Store new information
- `/query`: Query knowledge graph
- `/node/{id}`: Get node information
- `/evolution/{id}`: Get temporal evolution

### WebSocket Support
- Real-time updates
- Streaming responses
- Connection management

## Data Flow

### Ingestion Pipeline
```
Raw Data → Episode Creation → Entity Extraction → Relationship Detection → Graph Storage
   │              │                  │                    │                   │
   └──────────────┴──────────────────┴────────────────────┴───────────────────┘
                                 Temporal Tracking
```

### Query Pipeline
```
Query → Vector Search → Graph Traversal → Temporal Filtering → Response Generation
  │          │              │                  │                    │
  └──────────┴──────────────┴──────────────────┴────────────────────┘
                        Context Integration
```

## Security Architecture

### Authentication & Authorization
- AWS IAM integration
- Role-based access control
- API key management

### Data Protection
- Encryption at rest
- Encryption in transit
- Audit logging

## Monitoring & Observability

### Metrics
- Response latency
- Query performance
- Resource utilization
- Error rates

### Logging
- Application logs
- AWS CloudWatch integration
- Audit trails

## Scalability & Performance

### Optimization Strategies
- Connection pooling
- Query optimization
- Caching layers
- Batch processing

### Distribution
- Load balancing
- Horizontal scaling
- Resource management

## Development & Deployment

### CI/CD Pipeline
```
Code → Tests → Build → Security Scan → Deploy → Monitor
 │      │       │          │            │         │
 └──────┴───────┴──────────┴────────────┴─────────┘
              Automated Pipeline
```

### Environment Management
- Development
- Staging
- Production
- Disaster recovery

## References

- [Zep Paper](https://arxiv.org/abs/2501.13956)
- [AWS Documentation](https://docs.aws.amazon.com)
- [Neptune Documentation](https://docs.aws.amazon.com/neptune)
- [OpenSearch Documentation](https://opensearch.org/docs)

## Notes

- Architecture is designed for high availability and scalability
- Components are loosely coupled for maintainability
- AWS services are chosen for reliability and managed infrastructure
- Security is implemented at all layers
- Monitoring and observability are built-in 