# Datapath: type-safe structured paths

Provides simple macros and utilities for type-safe structured paths.
This is intended for use with S3 and [duckdb hive partitions](https://duckdb.org/docs/stable/data/partitioning/hive_partitioning), or simple S3 paths for [lancedb](https://lancedb.com).


## Basic Usage

```rust
use datapath::{datapath, Datapath};
use uuid::Uuid;

/// Define a datapath pattern
datapath! {
    struct CaptureRaw(capture/user_id=Uuid/ts=i64/raw/2.0);
}

// Create a datapath instance
let path = CaptureRaw {
    user_id: Uuid::new_v4(),
    ts: 1234567890,
};

println!("{}", path); // "capture/user_id=<uuid>/ts=1234567890/raw/2.0"

let file = path.with_file("data.json");
println!("{}", file); // "capture/user_id=<uuid>/ts=1234567890/raw/2.0/data.json"

let parsed = CaptureRaw::parse("capture/user_id=550e8400-e29b-41d4-a716-446655440000/ts=1234567890/raw/2.0/data.json");
match parsed {
    Some(datapath_file) => {
        println!("User ID: {}", datapath_file.path.user_id);
        println!("Timestamp: {}", datapath_file.path.ts);
        println!("File: {}", datapath_file.file);
    }
    None => println!("Invalid path format"),
}
```

## Schema Associations

Associate datapaths with schema types for type-safe data handling:

```rust
use datapath::datapath;

pub struct UserEvent {
    pub action: String,
    pub timestamp: i64,
}

datapath! {
    struct EventPath {
        pattern: events/user_id=String/date=i64/"v1.0"
        schema: UserEvent
    };
}

// EventPath now implements SchemaDatapath
// EventPath::Schema == UserEvent
```

## Examples

```rust
use datapath::datapath;

pub struct MetricsSchema;

datapath! {
    // Constant segments (identifiers)
    struct SimplePath(data/logs/events);

    // String literal constants (for segments with dashes)
    struct QuotedPath("web-data"/"user-logs"/2024);

    // Typed partitions with identifier keys
    struct TypedPath(domain/user_id=uuid::Uuid/timestamp=i64);

    // Typed partitions with dashes
    struct QuotedKeys("service-name"=String/"request-id"=uuid::Uuid);

    // With schema association
    struct MetricsData {
        pattern: metrics/service=String/timestamp=i64/"v1.0"
        schema: MetricsSchema
    };
}
```