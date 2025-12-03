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
use datapath::{datapath, Datapath};

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

## Pattern Introspection and Wildcards

Access the pattern string and work with wildcarded paths:

```rust
use datapath::{datapath, Datapath, Wildcardable};
use uuid::Uuid;

datapath! {
    struct Metrics(metrics/service=String/timestamp=i64/v1);
}

// Access the pattern string
// (use for logging/debug)
assert_eq!(Metrics::PATTERN, "metrics/service=String/timestamp=i64/v1");

// Convert to/from tuples
let metrics = Metrics {
    service: "api".to_string(),
    timestamp: 1234567890,
};
let tuple = metrics.clone().to_tuple();
assert_eq!(tuple, ("api".to_string(), 1234567890i64));

let recreated = Metrics::from_tuple(tuple);
assert_eq!(recreated.service, "api");
assert_eq!(recreated.timestamp, 1234567890);

// Create wildcarded paths for querying
let all_services = Metrics::from_wildcardable((
    Wildcardable::Star,
    Wildcardable::Value(1234567890i64),
));
assert_eq!(all_services, "metrics/service=*/timestamp=1234567890/v1");
```

## Examples

```rust
use datapath::{datapath, Datapath};

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

### Constant-Only Paths

Paths with no typed fields work correctly with empty tuples:

```rust
use datapath::{datapath, Datapath};

datapath! {
    struct ConstantPath(assets/data/"v1.0");
}

// PATTERN works for constant-only paths
assert_eq!(ConstantPath::PATTERN, "assets/data/v1.0");

// Tuple type is unit ()
let empty_tuple = ConstantPath {}.to_tuple();
assert_eq!(empty_tuple, ());

let path = ConstantPath::from_tuple(());
assert_eq!(format!("{}", path), "assets/data/v1.0");

// from_wildcardable also works (no wildcards possible)
let path_str = ConstantPath::from_wildcardable(());
assert_eq!(path_str, "assets/data/v1.0");
```