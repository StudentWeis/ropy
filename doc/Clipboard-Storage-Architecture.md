# Clipboard Record Storage Architecture

## Overview

The ropy project adopts a dual-layer storage architecture to manage clipboard records: persistent database storage and in-memory caching. This design ensures data persistence while providing good UI responsiveness.

## Storage Architecture

### Persistent Storage Layer (Database)

- **Storage Engine**: Uses `sled` embedded key-value database
- **Data Structure**: 
  - Primarily stored in the `clipboard_records` tree
  - Uses content hash as key to achieve automatic deduplication
  - Each record contains: ID, content, creation time, content type, pinned status
- **Auxiliary Index**: `TimeIndex` provides lightweight timestamp-based indexing, supporting efficient chronological queries

### In-Memory Cache Layer (Runtime)

- **Data Structure**: Vector of type `Arc<Mutex<Vec<ClipboardRecord>>>`
- **Purpose**: Provides fast access for the GUI interface, avoiding frequent database queries
- **Synchronization**: Kept synchronized with the database

## Data Synchronization Mechanism

### Adding Records
1. **Database**: Saved to the sled database via the `save()` method
2. **Memory**: Updates the record list in memory (if applicable)

### Deleting Records
1. **Database**: Removed from the sled database via the `delete()` method
2. **Memory**: Removes the corresponding record from the `records` vector in memory

```rust
self.records
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .retain(|record| record.id != id);
```

### Modifying Records (Pin/Unpin)
1. **Database**: Updates the pinned status in the database via `toggle_pin()`
2. **Memory**: Synchronously updates the pinned status of the corresponding record in memory

```rust
let mut guard = self.records.lock().unwrap_or_else(PoisonError::into_inner);
if let Some(record) = guard.iter_mut().find(|r| r.id == id) {
    record.pinned = !record.pinned;
}
```

## Query Operations

### Getting Recent Records
- Uses the `get_recent(limit)` method, primarily during initialization to load recent records from the database to the in-memory cache
- Leverages the `TimeIndex` to select record IDs
- Batch loads required records, avoiding deserialization of all data
- Sorted by pinned status and time (pinned records first, then reverse chronological order)

### Search Operation
- Implemented through the `filter_records_by_query(query)` method
- Filters text-type records in memory (after loading from database to cache)
- Performs case-insensitive keyword matching on text content
- Primarily used when users enter search keywords in the UI
- Empty queries return all cached records, non-empty queries return only matching text records

### UI Display Logic
- During initialization: Uses `get_recent` to load records from the database to the in-memory cache
- During search: Filters records in memory using `filter_records_by_query` method
- When not searching: Directly uses records from the in-memory cache without accessing the database

## Advantages

1. **Data Persistence**: All records are stored in the database, ensuring data is not lost after restart
2. **Performance Optimization**: Memory cache provides fast UI access
3. **Automatic Deduplication**: Content hash-based keys ensure identical content is not stored repeatedly
4. **Efficient Queries**: Time index supports fast chronological queries
5. **Data Consistency**: Add, delete, and modify operations update both the database and in-memory cache simultaneously

## Potential Issues

1. **Memory Usage**: Maintaining the complete record list in memory may consume significant memory for large amounts of history
2. **Synchronization Complexity**: Requires ensuring consistency between the database and in-memory cache
3. **Concurrency Safety**: Uses `Mutex` to protect shared data, requiring consideration of concurrent access performance impact
