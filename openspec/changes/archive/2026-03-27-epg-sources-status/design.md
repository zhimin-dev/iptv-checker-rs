## Context

The `GET /epg/sources` API currently returns a JSON array of EPG URLs. The frontend requires a way to know if the EPG data for the current day has been successfully downloaded. The data is stored in the `static/epg/{YYYY-MM-DD}` directory.

## Goals / Non-Goals

**Goals:**
- Modify the `GET /epg/sources` endpoint to return an object containing both the `list` of URLs and a `status` boolean.
- The `status` boolean should be `true` if `static/epg/{YYYY-MM-DD}` exists, and `false` otherwise.

**Non-Goals:**
- Validating the contents of the `static/epg/{YYYY-MM-DD}` folder (we only check if the folder itself exists).

## Decisions

1. **Response Structure**
   - **Decision:** Change the response from `Vec<String>` to a custom struct `EpgSourcesResponse { list: Vec<String>, status: bool }`.
   - **Rationale:** This provides a structured and extensible way to return the data and status together.

2. **Date Formatting & Path Checking**
   - **Decision:** Use `chrono::Local::now().format("%Y-%m-%d")` to get today's date, construct the path `static/epg/{date}`, and use `std::path::Path::new(&path).exists()` to check for existence.
   - **Rationale:** Standard, reliable way to check for directory existence in Rust.

## Risks / Trade-offs

- **Risk: Breaking API Change**
  - *Mitigation:* The frontend must be updated simultaneously to handle the new JSON object response instead of a plain array.
