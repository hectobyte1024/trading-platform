# Kafka Adapter

Production-grade event journal backed by Apache Kafka for distributed, fault-tolerant event storage.

## Features

- **Distributed Storage**: Events replicated across Kafka brokers
- **High Throughput**: Batch publishing with LZ4 compression
- **Full Replay**: Consume events from any point in time
- **Fault Tolerant**: Survives broker failures with configurable replication
- **Scalable**: Horizontal scaling via Kafka partitions

## Prerequisites

Install the required system library:

```bash
# Debian/Ubuntu
sudo apt install libsasl2-dev

# macOS
brew install libsasl2

# RHEL/CentOS
sudo yum install cyrus-sasl-devel
```

## Usage

### Basic Setup

```rust
use kafka_adapter::KafkaJournal;
use event_journal::EventJournal;

#[tokio::main]
async fn main() -> Result<()> {
    // Create Kafka-backed journal
    let journal = KafkaJournal::new(
        "localhost:9092",           // Kafka brokers
        "trading-events",            // Topic name
        "matching-engine-group"      // Consumer group
    ).await?;

    // Use with matching engine
    let engine = MatchingEngine::new(
        Arc::new(journal),
        Arc::new(risk_engine)
    );

    Ok(())
}
```

### Replay from Kafka

```rust
// Create new engine and replay all events
let engine = MatchingEngine::new_with_replay(
    Arc::new(journal),
    Arc::new(risk_engine)
).await?;
```

### Direct Producer/Consumer

```rust
use kafka_adapter::{KafkaProducer, KafkaConsumer};

// Producer
let producer = KafkaProducer::new("localhost:9092", "my-topic")?;
producer.publish(Some("key"), b"payload").await?;
producer.flush().await?;

// Consumer
let consumer = KafkaConsumer::new(
    "localhost:9092",
    "my-group",
    "my-topic"
)?;

consumer.consume_from_beginning(|payload| {
    // Process message
    println!("Received: {:?}", payload);
    Ok(())
}).await?;
```

## Kafka Configuration

The adapter uses the following default settings:

- **Compression**: LZ4
- **Batch Size**: 16384 bytes
- **Linger Time**: 10ms (for batching)
- **Acks**: all (wait for all replicas)
- **Auto Offset Reset**: earliest
- **Auto Commit**: enabled

## Running Kafka Locally

### Using Docker

```bash
# Start Kafka with Zookeeper
docker-compose up -d

# Create topic
docker exec -it kafka kafka-topics.sh \
    --create \
    --topic trading-events \
    --bootstrap-server localhost:9092 \
    --partitions 3 \
    --replication-factor 1
```

### docker-compose.yml

```yaml
version: '3'
services:
  zookeeper:
    image: confluentinc/cp-zookeeper:latest
    environment:
      ZOOKEEPER_CLIENT_PORT: 2181
      ZOOKEEPER_TICK_TIME: 2000

  kafka:
    image: confluentinc/cp-kafka:latest
    depends_on:
      - zookeeper
    ports:
      - "9092:9092"
    environment:
      KAFKA_BROKER_ID: 1
      KAFKA_ZOOKEEPER_CONNECT: zookeeper:2181
      KAFKA_ADVERTISED_LISTENERS: PLAINTEXT://localhost:9092
      KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR: 1
```

## Production Deployment

For production use:

1. **Multiple Brokers**: Deploy at least 3 Kafka brokers
2. **Replication Factor**: Set to 3 for fault tolerance
3. **Partitions**: Use multiple partitions for scalability
4. **Monitoring**: Enable Kafka metrics and monitoring
5. **Retention**: Configure retention based on compliance needs

```bash
# Create production topic
kafka-topics.sh \
    --create \
    --topic trading-events \
    --bootstrap-server broker1:9092,broker2:9092,broker3:9092 \
    --partitions 12 \
    --replication-factor 3 \
    --config retention.ms=604800000  # 7 days
```

## Testing

Tests require a running Kafka instance and are marked with `#[ignore]`:

```bash
# Run with ignored tests (requires Kafka on localhost:9092)
cargo test -p kafka-adapter -- --ignored
```

## Performance

Based on Kafka benchmarks:

- **Write Throughput**: 100K+ events/sec
- **Read Throughput**: 200K+ events/sec
- **Latency**: ~5-10ms (p99)
- **Storage**: Compression reduces size by ~60-70%

## Troubleshooting

### Connection Refused

Ensure Kafka is running and accessible:
```bash
telnet localhost 9092
```

### Consumer Not Receiving Messages

Check consumer group status:
```bash
kafka-consumer-groups.sh \
    --bootstrap-server localhost:9092 \
    --describe \
    --group matching-engine-group
```

### Topic Not Found

List available topics:
```bash
kafka-topics.sh \
    --list \
    --bootstrap-server localhost:9092
```
