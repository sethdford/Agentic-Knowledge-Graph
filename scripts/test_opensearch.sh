#!/bin/bash

set -e

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "Docker is not running. Please start Docker and try again."
    exit 1
fi

# Stop and remove existing container if it exists
docker rm -f opensearch-test 2>/dev/null || true

# Start OpenSearch container
echo "Starting OpenSearch container..."
docker run -d \
    --name opensearch-test \
    -p 9200:9200 \
    -p 9600:9600 \
    -e "discovery.type=single-node" \
    -e "plugins.security.disabled=true" \
    opensearchproject/opensearch:2.11.1

# Wait for OpenSearch to be ready
echo "Waiting for OpenSearch to be ready..."
until curl -s http://localhost:9200 > /dev/null; do
    echo "Waiting for OpenSearch to start..."
    sleep 1
done
echo "OpenSearch is ready!"

# Run the tests
cargo test $@

# Stop and remove the container
docker stop opensearch-test
docker rm opensearch-test

echo "Tests completed." 