# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Essential Commands

```bash
# Build
cargo build

# Test
cargo test

# Run benchmarks (requires nightly)
cargo +nightly bench

# Run examples
cargo run --example xml-analyze -- file.xml
cargo run --example print_events -- file.xml
cargo run --example async_reader --features async

# Lint and format
cargo fmt
cargo clippy
```

## High-Level Architecture

### Pull Parser Design
The library implements a StAX-style pull parser where the application drives the parsing process. The core `EventReader` in `src/reader.rs` yields `XmlEvent` instances as it parses, avoiding DOM construction for memory efficiency.

### State Machine Architecture
The parser uses a sophisticated state machine with context-specific handlers in `src/reader/parser/`:
- Each parsing context (CDATA, comments, declarations, etc.) has its own module
- The main parser (`src/reader/parser.rs`) coordinates state transitions
- Lexical analysis is separated into `src/reader/lexer.rs`

### Event-Driven Design
Both reading and writing use the `XmlEvent` enum as the central abstraction:
- Reader: Pulls events from XML source
- Writer: Pushes events to generate XML output
- This enables streaming transformations without memory overhead

### Namespace Management
The library maintains a namespace stack during parsing/writing:
- Automatic scope tracking and resolution
- Configurable namespace handling via `ParserConfig` and `EmitterConfig`
- Proper handling of default namespaces and prefixes

### Security Considerations
Built-in protections against XML attacks:
- Entity expansion limits (depth: 10, length: 1MB by default)
- No unsafe code (`#![forbid(unsafe_code)]`)
- Designed for untrusted input with configurable resource limits

### Key Configuration Points
- `ParserConfig`: Controls parsing behavior (whitespace, comments, CDATA handling)
- `EmitterConfig`: Controls output formatting (indentation, empty elements)
- Both use builder pattern for ergonomic configuration

### Testing Strategy
- Unit tests throughout modules
- XML conformance tests in `tests/`
- Benchmarks in `benches/` for performance tracking
- Examples serve as integration tests

## Important Implementation Details

### Buffer Requirements
The parser requires buffered input for performance - always wrap readers in `BufReader`.

### Error Handling
Custom error types with proper context - check `Error` enum for all possible failure modes.

### No External Dependencies
Pure Rust implementation with zero runtime dependencies - all functionality is self-contained.

## Async Support

The library supports async XML parsing via the `async` feature flag with **99% code reuse** between sync and async implementations.

### Architecture Overview

The async design abstracts only I/O operations while preserving all parsing logic:

```
                    ┌─────────────────┐
                    │   EventReader   │ (sync API)
                    │  (uses Read)    │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │  PullParser<R>  │ (generic parser)
                    │ (state machine) │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │   Lexer<R>      │ (generic lexer)
                    └────────┬────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
     ┌────────▼────────┐          ┌────────▼────────┐
     │  SyncReader<R>  │          │ AsyncReader<R>  │
     │  (impl XmlRead) │          │(impl AsyncXmlRead)│
     └─────────────────┘          └─────────────────┘
```

### Key Components

#### I/O Abstraction Traits
- **`XmlRead`**: Synchronous character reading trait
- **`AsyncXmlRead`**: Asynchronous character reading trait  
- Same interface, only `async fn` vs `fn` difference

#### Reader Adapters
- **`SyncReader<R>`**: Wraps `std::io::Read` types
- **`AsyncReader<R>`**: Wraps `tokio::io::AsyncRead` types
- Both handle character encoding and buffering identically

#### Generic Parser Core
- **`PullParser<R>`**: Generic over reader type
- **`Lexer<R>`**: Generic tokenizer 
- **Zero logic changes**: Only I/O operations differ

### Character Reading Abstraction

Character decoding logic is abstracted into pure functions:

```rust
// Synchronous version
pub(crate) fn read_char_from<R: Read>(...) -> Result<Option<char>, CharReadError>

// Asynchronous version  
pub(crate) async fn async_read_char_from<R: AsyncRead + Unpin>(...) -> Result<Option<char>, CharReadError>
```

The async version is a line-by-line port with only I/O operations changed to use `.await`.

### Usage Examples

#### Basic Setup
```rust
// Enable in Cargo.toml
[dependencies]
xml = { version = "1.0", features = ["async"] }
```

#### Async Parsing
```rust
use xml::AsyncEventReader;
use tokio::fs::File;
use tokio::io::BufReader;

// Read from async file
let file = File::open("document.xml").await?;
let buf_reader = BufReader::new(file);
let mut reader = AsyncEventReader::new(buf_reader);

while let Ok(event) = reader.next().await {
    match event {
        XmlEvent::StartElement { name, .. } => {
            println!("Element: {}", name.local_name);
        }
        XmlEvent::EndDocument => break,
        _ => {}
    }
}
```

#### With Configuration
```rust
use xml::{AsyncEventReader, ParserConfig};

let config = ParserConfig::new()
    .trim_whitespace(true)
    .cdata_to_characters(true);
    
let mut reader = AsyncEventReader::new_with_config(async_source, config);
```

### Testing Structure

Async tests mirror sync tests exactly:
- **`tests/async_event_reader.rs`**: Async equivalents of sync reader tests
- **`tests/async_encoding.rs`**: Async encoding handling tests
- **`tests/async_error_handling.rs`**: Async error scenario tests
- **`tests/async_streaming.rs`**: Large document and streaming tests

### Design Benefits

1. **Code Reuse**: 99% shared logic between sync and async
2. **Type Safety**: Compile-time prevention of sync/async mixing
3. **Zero Runtime Overhead**: Generic abstraction with no performance cost
4. **Backward Compatible**: Original sync API unchanged
5. **Memory Efficient**: Same streaming approach for both sync and async
6. **Easy Testing**: Parser logic testable with mock readers

### Error Handling

Both sync and async versions use the same error types:
- **`CharReadError`**: I/O and encoding errors
- **`Error`**: Parser-level errors with position information
- **`SyntaxError`**: XML syntax violations

Async contexts properly propagate errors through `Result<T, E>` types.