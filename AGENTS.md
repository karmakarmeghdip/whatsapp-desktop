# Agent Guidelines for WhatsApp Desktop

This document provides comprehensive guidance for AI agents working on the WhatsApp Desktop codebase.

## Project Overview

WhatsApp Desktop is a Rust-based desktop application using the [Iced](https://iced.rs/) GUI framework. It provides a WhatsApp Web alternative with native desktop integration using the `whatsapp-rust` library.

### Key Technologies
- **GUI Framework**: Iced (Elm Architecture pattern)
- **WhatsApp Library**: `whatsapp-rust` with tokio runtime
- **Storage**: SQLite with WAL mode
- **Async Runtime**: Tokio
- **Architecture**: Model-View-Controller (MVC) with RPC layer

## Codebase Structure

```
src/
├── main.rs                 # Application entry point
├── app.rs                  # Iced application setup
├── controller.rs           # Message handling and state updates (MVC Controller)
├── model.rs                # Re-exports model modules
├── model/
│   ├── chat.rs            # Chat and message types
│   ├── connection.rs      # Connection state types
│   └── state.rs           # Central application state (keep <250 LOC)
├── view.rs                # Re-exports view modules
├── view/
│   ├── main_view.rs       # Main view composition
│   ├── sidebar.rs         # Chat list sidebar
│   ├── chat.rs            # Chat message view
│   ├── loading.rs         # Loading screen
│   ├── pairing.rs         # QR code pairing screen
│   └── settings.rs        # Settings view
├── whatsapp/              # WhatsApp integration module
│   ├── mod.rs             # Module exports
│   ├── client.rs          # WhatsApp client connection (keep <250 LOC)
│   ├── events.rs          # Event types
│   ├── storage.rs         # Storage module (refactored into submodules)
│   ├── storage/           # Storage submodules
│   │   ├── schema.rs      # Database schema
│   │   ├── models.rs      # Storage models
│   │   ├── queries.rs     # Query operations
│   │   └── writer.rs      # Background writer
│   └── types.rs           # Core WhatsApp types
└── rpc/                   # RPC communication layer
    ├── mod.rs             # RPC types and requests
    ├── client.rs          # RPC client
    ├── service.rs         # RPC service implementation (keep <250 LOC)
    └── types.rs           # RPC type definitions
```

### Architecture Pattern

The application follows the **Model-View-Controller (MVC)** pattern with an **RPC Layer**:

1. **Model** (`src/model/`): Pure data structures and state management
2. **View** (`src/view/`): Iced widgets and UI composition
3. **Controller** (`src/controller.rs`): Handles user actions and updates state
4. **RPC Layer** (`src/rpc/`): Communication between UI and WhatsApp service
5. **WhatsApp Module** (`src/whatsapp/`): WhatsApp-specific integration

## Reference Documentation

Critical reference materials are located in the `reference/` directory:

### 1. Iced Examples (`reference/iced/`)
**Symlink to**: `/home/mkarmakar/Documents/iced/examples`

Contains official Iced framework examples. Essential for understanding:
- Widget usage patterns
- Subscription handling
- State management
- Custom styling

**Key Examples to Reference**:
- `counter/` - Basic state management
- `todos/` - List handling and user input
- `websocket/` - Async communication patterns
- `custom_widget/` - Creating custom components

### 2. WhatsApp-Rust Documentation

#### Full Reference (`reference/whatsapp.txt`)
Comprehensive documentation for the `whatsapp-rust` library:
- All public APIs and methods
- Event handling
- Message types
- Connection management
- **Use this for**: Deep understanding of available functionality

#### Quick Reference (`reference/whatsapp-short.txt`)
Condensed reference with most common patterns:
- Common event handlers
- Typical message flows
- Quick lookup for frequently used types
- **Use this for**: Fast lookups during implementation

## Code Organization Policy

### Module Size Limit: 250 Lines of Code

**CRITICAL RULE**: When any module exceeds 250 lines of code (LOC), it MUST be split into submodules.

#### Why 250 LOC?
- **Cognitive Load**: Easier to understand and reason about
- **Testability**: Smaller units are easier to test
- **Maintainability**: Changes are localized
- **Code Review**: Faster and more thorough reviews
- **Navigation**: Easier to find specific functionality

#### Refactoring Strategy

When a file exceeds 250 LOC:

1. **Identify Responsibilities**: Look for distinct functional areas
2. **Create Submodules**: Split into logical submodules
3. **Re-export in Parent**: Maintain API compatibility
4. **Update Imports**: Ensure all references still work
5. **Run Tests**: Verify everything compiles and works

#### Example: Storage Module Refactoring

**Before**: `src/whatsapp/storage.rs` (379 lines)

**After**:
```
src/whatsapp/
├── storage.rs           (102 lines - public API)
└── storage/
    ├── schema.rs        (119 lines - database schema)
    ├── models.rs        (data models)
    ├── queries.rs       (162 lines - query operations)
    └── writer.rs        (212 lines - background writer)
```

**Parent Module Pattern**:
```rust
// src/whatsapp/storage.rs
//! WhatsApp Storage Module

// Sub-modules
mod models;
mod queries;
mod schema;
mod writer;

// Re-export public types
pub use models::{StoredMessage, StorageWriter};
pub use queries::load_snapshot;
pub use writer::spawn_writer;

// Public API functions...
```

### High Cohesion, Low Coupling

#### High Cohesion
- Each module should have a **single, well-defined responsibility**
- Related functionality should be grouped together
- Example: `storage/schema.rs` only handles database schema

#### Low Coupling
- Modules should depend on abstractions, not concrete implementations
- Minimize cross-module dependencies
- Use traits and interfaces where appropriate
- Example: `ChatManager` doesn't know about `MessageManager` internals

### Module Organization Best Practices

1. **Module Documentation**: Every module must start with a doc comment:
   ```rust
   //! Module Name
   //!
   //! Brief description of what this module does.
   ```

2. **Public API Surface**: Keep the public API minimal:
   ```rust
   // Good: Re-export only what's needed
   pub use models::{StoredMessage, StorageWriter};
   
   // Bad: Exporting everything
   pub use models::*;
   ```

3. **Error Handling**: Use `Result` types and the `?` operator:
   ```rust
   pub fn load_data(path: &Path) -> Result<Data, Box<dyn std::error::Error>> {
       let content = fs::read_to_string(path)?;
       let data = parse(&content)?;
       Ok(data)
   }
   ```

4. **Type Safety**: Leverage Rust's type system:
   ```rust
   // Use newtypes for domain concepts
   pub struct Jid(pub String);
   
   // Use enums for state machines
   pub enum ConnectionState {
       Disconnected,
       Connecting,
       Connected,
       // ...
   }
   ```

## Rust Best Practices

### 1. Naming Conventions

- **Modules**: `snake_case` (`chat_manager`)
- **Types/Structs/Enums**: `PascalCase` (`ChatManager`, `ConnectionState`)
- **Functions/Methods**: `snake_case` (`update_chat`)
- **Constants**: `SCREAMING_SNAKE_CASE` (`MAX_RETRY_COUNT`)
- **Traits**: `PascalCase` (`StorageBackend`)

### 2. Error Handling

Always use proper error handling:

```rust
// Good: Propagate errors
fn load_file(path: &Path) -> Result<String, io::Error> {
    fs::read_to_string(path)
}

// Good: Handle errors explicitly
match result {
    Ok(data) => process(data),
    Err(e) => {
        log::error!("Failed to process: {}", e);
        default_value
    }
}

// Good: Use Option methods
let value = maybe_value.unwrap_or_default();
```

### 3. Documentation

Document all public items:

```rust
/// Represents a WhatsApp chat conversation.
///
/// # Examples
///
/// ```rust,ignore
/// let chat = Chat::new(jid, "Family Group");
/// ```
pub struct Chat {
    /// Unique JID identifier
    pub jid: Jid,
    /// Display name
    pub name: String,
}

impl Chat {
    /// Creates a new chat with the given JID and name.
    ///
    /// # Arguments
    ///
    /// * `jid` - The WhatsApp JID
    /// * `name` - The display name
    pub fn new(jid: Jid, name: impl Into<String>) -> Self {
        // ...
    }
}
```

### 4. Ownership and Borrowing

Prefer borrowing over cloning:

```rust
// Good: Borrow
fn process_chat(chat: &Chat) { }

// Bad: Clone unnecessarily
fn process_chat(chat: Chat) { }

// Good: Clone when needed
let chat_copy = chat.clone();
```

### 5. Iterator Patterns

Use iterator methods for clarity:

```rust
// Good: Iterator chain
let unread_count = chats
    .iter()
    .filter(|c| !c.is_muted)
    .map(|c| c.unread_count)
    .sum::<u32>();

// Avoid: Manual loops for simple operations
let mut count = 0;
for chat in chats {
    if !chat.is_muted {
        count += chat.unread_count;
    }
}
```

### 6. Async/Await

Follow async best practices:

```rust
// Good: Propagate async
async fn fetch_data() -> Result<Data, Error> {
    client.get_data().await
}

// Good: Spawn tasks appropriately
tokio::spawn(async move {
    process_events(rx).await;
});

// Good: Use select! for multiple async operations
tokio::select! {
    Some(event) = event_rx.recv() => handle_event(event),
    Some(cmd) = cmd_rx.next() => handle_command(cmd),
    _ = shutdown_signal => break,
}
```

### 7. Testing

Write unit tests for complex logic:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_ordering() {
        let mut manager = MessageManager::new();
        // Test implementation...
    }
}
```

## Implementation Guidelines

### Adding New Features

1. **Check File Sizes**: Before adding code, check if any file is approaching 250 LOC
2. **Plan Module Structure**: If splitting is needed, plan the new module structure
3. **Write Tests**: Add unit tests for new functionality
4. **Update Documentation**: Document new public APIs
5. **Run Checks**: Ensure `cargo check` and `cargo clippy` pass

### Refactoring Existing Code

When refactoring:

1. **Preserve API**: Keep the public API stable when possible
2. **Incremental Changes**: Make small, focused changes
3. **Test Coverage**: Maintain or improve test coverage
4. **Documentation**: Update docs to reflect changes

### Working with Iced

1. **Reference Examples**: Check `reference/iced/` for patterns
2. **Subscription Pattern**: Use subscriptions for async operations
3. **State Management**: Keep state updates pure (no side effects in update)
4. **View Functions**: Make view functions deterministic based on state

### Working with WhatsApp-Rust

1. **Check References**: Consult `reference/whatsapp.txt` or `reference/whatsapp-short.txt`
2. **Event Handling**: Handle all relevant events in the event handler
3. **Error Recovery**: Implement reconnection logic for connection failures
4. **Storage**: Persist important data using the storage module

## Common Patterns

### State Management

```rust
// Model state changes immutably when possible
pub fn update_message(&self, id: &str, status: Status) -> Self {
    let mut new_state = self.clone();
    new_state.messages.update(id, status);
    new_state
}
```

### Event Handling

```rust
// Handle events with pattern matching
match event {
    WhatsAppEvent::MessageReceived(msg) => {
        state.add_message(msg);
    }
    WhatsAppEvent::Connected => {
        state.set_connected();
    }
    _ => {}
}
```

### Async Operations

```rust
// Use spawn for background tasks
tokio::spawn(async move {
    while let Some(event) = rx.recv().await {
        process_event(event).await;
    }
});
```

## Verification Checklist

Before completing any task:

- [ ] Code compiles (`cargo check` passes)
- [ ] No file exceeds 250 LOC (unless unavoidable)
- [ ] Public APIs are documented
- [ ] Complex logic has unit tests
- [ ] Error handling is comprehensive
- [ ] No unnecessary clones
- [ ] Follows Rust naming conventions
- [ ] References appropriate documentation

## Tools and Commands

### Essential Commands

```bash
# Check compilation
cargo check

# Build the project
cargo build

# Run with logging
RUST_LOG=info cargo run

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy

# Check file sizes
find src -name "*.rs" -exec wc -l {} + | sort -rn
```

### Finding Large Files

To identify files that need splitting:

```bash
find src -name "*.rs" -exec wc -l {} + | sort -rn | head -20
```

Files exceeding 250 LOC should be considered for refactoring.

## Summary

1. **Keep modules small** (<250 LOC)
2. **Split into submodules** when size limit is reached
3. **Maintain high cohesion** (single responsibility)
4. **Minimize coupling** (few dependencies)
5. **Follow Rust best practices** (naming, error handling, ownership)
6. **Reference documentation** (Iced examples, whatsapp-rust docs)
7. **Write tests** for complex logic
8. **Document public APIs**

Remember: Small, focused modules are easier to understand, test, and maintain!
