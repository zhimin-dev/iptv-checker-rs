## Context

The application downloads and caches EPG data in a date-based directory structure (e.g., `static/epg/{YYYY-MM-DD}`). We need an API endpoint to allow clients to delete this folder, forcing the system to re-download or clear out corrupted/outdated data for the current day.

## Goals / Non-Goals

**Goals:**
- Provide a `DELETE /epg/cache` endpoint.
- Safely delete the `static/epg/{YYYY-MM-DD}` directory and all its contents.
- Return appropriate JSON responses for success, failure, or if the directory doesn't exist.

**Non-Goals:**
- Deleting historical EPG caches (older dates). This endpoint is specifically for the current day's cache.
- Clearing the in-memory `GLOBAL_EPG_CACHE` (the sync process will overwrite it anyway, but we might want to clear it if needed. For now, just deleting the file system cache is sufficient to trigger a fresh download on the next sync).

## Decisions

1. **Endpoint Path & Method**
   - **Decision:** `DELETE /epg/cache` (or `POST /epg/cache/delete` if strict REST isn't required, but `DELETE` is more semantic). We will use `DELETE /epg/cache`.
   - **Rationale:** Standard RESTful approach for removing a resource.

2. **File Deletion Method**
   - **Decision:** Use `std::fs::remove_dir_all`.
   - **Rationale:** The cache is a directory containing XML files. `remove_dir_all` recursively deletes the directory and its contents. It returns an error if the path doesn't exist, which we can handle gracefully.

## Risks / Trade-offs

- **Risk: Concurrent Access**
  - *Mitigation:* If the background sync task is currently writing to this directory while the delete API is called, there could be an I/O conflict. Since the sync task is relatively fast, this is a minor risk. We will rely on standard OS file locking/errors and return a 400 Bad Request if deletion fails.
