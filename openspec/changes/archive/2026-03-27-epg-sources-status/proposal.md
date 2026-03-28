## Why

Currently, the `GET /epg/sources` API only returns the list of EPG URLs from the configuration. Clients need a way to know if today's EPG data has been successfully downloaded and is available locally. By adding a status indicator based on the existence of today's EPG folder, the frontend can easily display whether the EPG data is ready or if a sync is needed.

## What Changes

- Modify the `GET /epg/sources` API response structure. Instead of returning a plain array of strings, it will return an object containing the list of sources and a boolean `status` flag.
- The `status` flag will be `true` if the folder `static/epg/{YYYY-MM-DD}` (where `{YYYY-MM-DD}` is today's date) exists, and `false` otherwise.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `epg-data-management`: Modifying the `GET /epg/sources` API requirement to include the `status` field based on the existence of today's EPG directory.

## Impact

- **API Response Format**: The `GET /epg/sources` endpoint will return a JSON object `{"list": [...], "status": true/false}` instead of just a JSON array `[...]`. This is a **BREAKING** change for clients consuming this specific endpoint.
- **Affected Code**: `src/web.rs` (the `get_epg_sources` handler) and potentially `src/utils.rs` or `src/search.rs` to check the directory existence.
