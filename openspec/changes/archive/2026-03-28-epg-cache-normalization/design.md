## Context

The application parses external EPG XML files and caches the data in memory (`GLOBAL_EPG_CACHE`) to quickly serve EPG data for matched M3U channels. Recently, a new `epg_mapping` feature was introduced to map M3U channels to standardized EPG `tvg-id`s based on region priority. However, the EPG cache currently stores the raw, unmapped channel IDs and display names (which may be in Traditional Chinese). To ensure the cached EPG data correctly aligns with the M3U generation and API queries, the EPG data must be normalized *before* it is inserted into the cache.

## Goals / Non-Goals

**Goals:**
- Update the EPG XML parsing logic (e.g., in `src/epg_xml.rs` or `src/web.rs`) to convert `display-name` values from Traditional to Simplified Chinese using the existing `trad_to_simp` function.
- Update the EPG XML parsing logic to map the raw `channel` ID to a standardized ID using `crate::epg_mapping::get_best_tvg_id`.
- Ensure that `programme` entries are also updated so their `channel` attribute matches the newly mapped standardized ID.
- Store this normalized data in `GLOBAL_EPG_CACHE`.

**Non-Goals:**
- Modifying the original EPG XML files on disk.
- Changing the structure of `GLOBAL_EPG_CACHE`.
- Changing how M3U files are generated (this was handled in a previous change).

## Decisions

1. **Where to apply normalization**:
   - **Decision**: Apply normalization during the parsing phase in `src/epg_xml.rs` (specifically within `parse_epg_xml` or the function responsible for building the cache).
   - **Rationale**: Normalizing data at the point of ingestion ensures that all downstream consumers (APIs, M3U generators) interact with a consistent, standardized dataset without needing to apply transformations on the fly.

2. **Handling `display-name`**:
   - **Decision**: For each `<channel>`, extract the `display-name`, pass it through `crate::common::translate::trad_to_simp`, and store the simplified string.

3. **Handling `channel` IDs**:
   - **Decision**: Use the simplified `display-name` to look up the standardized ID via `crate::epg_mapping::get_best_tvg_id(None, &simplified_name)`. Replace the original `channel` ID with this standardized ID.
   - **Rationale**: This ensures that regional variants (e.g., `CCTV-1.cn`, `CCTV-1.hk`) are all mapped to the same preferred ID (e.g., `CCTV-1.cn`), effectively merging their data in the cache.

4. **Handling `programme` entries**:
   - **Decision**: When parsing `<programme>` tags, we need to know the standardized ID for the program's original `channel` attribute. We will maintain a temporary mapping (`HashMap<String, String>`) during parsing that maps the original XML channel ID to the standardized ID. When a `<programme>` is parsed, its `channel` attribute will be replaced using this temporary map.
   - **Rationale**: Programs only reference the channel ID. Without this temporary map, we wouldn't know which standardized ID to assign to a program.

## Risks / Trade-offs

- **Data Overlap/Duplication**: If multiple regional variants of a channel are mapped to the same standardized ID, their programs will be merged under the same ID in the cache. This could lead to overlapping or duplicate programs for the same time slots.
  - *Mitigation*: For now, we accept this behavior as it provides *some* EPG data rather than none. Future enhancements could implement deduplication logic based on start/stop times.
- **Performance Overhead**: Applying string translation and mapping lookups during parsing will increase the time it takes to load the EPG cache.
  - *Mitigation*: The `trad_to_simp` function and `HashMap` lookups are relatively fast. Since parsing happens asynchronously in the background (e.g., via a scheduled task), the impact on user-facing API latency is negligible.