## Why

When parsing and caching EPG XML data into `GLOBAL_EPG_CACHE`, the raw `channel` IDs and `display-name`s are stored directly. This creates a mismatch when M3U files use mapped IDs (via the newly introduced `epg_mapping`) or simplified Chinese names. To ensure M3U channels match seamlessly with the cached EPG data, we need to normalize the cached data by converting display names to simplified Chinese and mapping channel IDs using the same `epg_mapping` logic.

## What Changes

- Update the EPG XML parsing logic to apply `trad_to_simp` to all channel `display-name`s before they are stored in the cache.
- Update the EPG XML parsing logic to map the raw `channel` ID to the standardized `tvg-id` using `epg_mapping::get_best_tvg_id` based on the simplified display name.
- Ensure that all `programme` entries in the EPG XML also have their `channel` attribute updated to match the newly mapped standardized `tvg-id`, so that programs link correctly to their channels.

## Capabilities

### New Capabilities
- `epg-cache-normalization`: Defines the normalization rules (Traditional to Simplified conversion and ID mapping) applied to EPG data before it is cached.

### Modified Capabilities
- `epg-data-management`: The caching behavior is modified to store normalized data instead of raw XML data.

## Impact

- **EPG Parsing**: The parsing process (`src/epg_xml.rs`) will have a slightly higher CPU overhead due to string conversion and mapping lookups.
- **Cache Consistency**: The `GLOBAL_EPG_CACHE` will now contain standardized channel IDs, which means any existing API endpoints querying the cache must use the standardized IDs.
- **Data Deduplication**: Multiple regional variants of the same channel (e.g., `CCTV-1.cn`, `CCTV-1.hk`) might be mapped to the same standardized ID, effectively merging their program schedules in the cache.