# Project Overview
**Name**: iptv-checker-rs
**Description**: A high-performance IPTV playlist (M3U) checker and EPG (XMLTV) manager written in Rust. It validates stream availability, parses and generates EPG XML data, and provides a web interface/API.

# Tech Stack
- **Language**: Rust (Edition 2021)
- **Async Runtime**: `tokio` (full features, macros)
- **Web Framework**: `actix-web`, `actix-rt`, `actix-files`, `actix-multipart`
- **HTTP Client**: `reqwest`
- **Serialization/Deserialization**: `serde`, `serde_json`
- **XML Processing**: `quick-xml` (with serialization support)
- **CLI Parsing**: `clap`
- **Date & Time**: `chrono`
- **Compression/Archive**: `zip`, `flate2`
- **Task Scheduling**: `clokwerk`
- **Logging**: `log`, `simplelog`

# Architecture & Domain Knowledge
- **M3U Parsing**: Handles standard and extended M3U playlists, extracting channel names, URLs, and metadata.
- **EPG Management**: Parses XMLTV format files (`<tv>`, `<channel>`, `<programme>`), converts them to JSON, and can serialize Rust objects back into valid XMLTV files. Supports downloading and extracting zipped/gzipped EPG data.
- **Stream Checking**: Concurrently verifies the availability and health of IPTV streams using async HTTP requests with timeouts.
- **Web API**: Exposes endpoints via Actix-Web for managing configurations, tasks, and serving results.

# Coding Conventions & Best Practices
- **Naming**: `snake_case` for variables, functions, and modules; `PascalCase` for types, structs, and traits.
- **Async/Concurrency**:
  - Heavily leverage `tokio` for I/O bound tasks.
  - Use `tokio::spawn` for background tasks and structured concurrency.
  - Use `tokio::sync::mpsc` for message passing and `tokio::sync::Mutex`/`RwLock` for shared state.
  - Avoid blocking operations in async contexts; offload to blocking threads if necessary.
- **Error Handling**:
  - Use Rust's `Result` and `Option` types extensively.
  - Propagate errors using the `?` operator.
  - Define clear custom error types where appropriate.
- **Code Organization**:
  - Highly modular: separate concerns into distinct modules (e.g., `common`, `config`, `search`, `epg_xml`, `utils`).
  - Use expressive variable names (e.g., `is_ready`, `has_data`).
- **Performance**:
  - Minimize async overhead; use synchronous code where async isn't strictly needed.
  - Optimize data structures to reduce lock contention and duration.