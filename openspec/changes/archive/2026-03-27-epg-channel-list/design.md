## Context

The application parses and caches EPG XMLTV data into memory (`GLOBAL_EPG_CACHE`). Currently, clients can query programs for a specific channel name, but there is no endpoint to discover *which* channels are available in the cache.

## Goals / Non-Goals

**Goals:**
- Provide a `GET /epg/channel-list` endpoint.
- Return a JSON list of all available channels in the cache, formatted as `{"list": [{"name": "cctv", "channel": "1"}, ...]}`.

**Non-Goals:**
- Returning the full program data for all channels (this would be too large).
- Returning channels that are in the config but not successfully parsed into the cache.

## Decisions

1. **Data Source**
   - **Decision:** Extract the channel list directly from `GLOBAL_EPG_CACHE`. Since the cache currently uses `HashMap<String, Vec<Programme>>` where the key is the channel name, we can easily get the names.
   - **Rationale:** This ensures the endpoint accurately reflects what is currently available in memory.

2. **Channel ID mapping**
   - **Decision:** The requested format includes `"channel": "1"`. However, our current `GLOBAL_EPG_CACHE` only stores the `channel_name` as the key and a list of `Programme`s. The `Programme` struct has a `channel` field which corresponds to the original channel ID from the XML. We can extract this ID from the first `Programme` in the list for each channel.
   - **Rationale:** Avoids modifying the global cache structure while still satisfying the API requirement.

## Risks / Trade-offs

- **Risk: Performance on large caches**
  - *Mitigation:* Iterating over the keys of the `HashMap` and extracting the first item's `channel` field is very fast (O(N) where N is the number of channels, typically < 10,000). This will not block the async executor significantly.
