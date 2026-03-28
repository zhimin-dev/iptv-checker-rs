## Why

Currently, the EPG data is parsed and cached in memory (`GLOBAL_EPG_CACHE`), and we can query programs for a specific channel. However, there is no way for the frontend or clients to know *which* channels actually have EPG data available. Providing a `/epg/channel-list` endpoint allows clients to retrieve a summary list of all available EPG channels, making it easier to build UI dropdowns or matching logic.

## What Changes

- Add a new REST API endpoint `GET /api/epg/channel-list` (or `GET /epg/channel-list` depending on routing setup).
- The endpoint will iterate over the keys (channel names) in the `GLOBAL_EPG_CACHE`.
- It will return a JSON object in the format `{"list": [{"name": "cctv", "channel": "1"}, ...]}`. Note: Since our cache currently uses the channel name as the key, we might need to adjust the cache structure or extract the original channel ID if required, or simply return the name.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `epg-data-management`: Adding a new requirement for an API endpoint to list all available EPG channels from the cache.

## Impact

- **New API Endpoint**: A new route will be added to `src/web.rs`.
- **Cache Access**: A new helper function will be added to `src/epg_xml.rs` to extract the list of channels from `GLOBAL_EPG_CACHE`.
