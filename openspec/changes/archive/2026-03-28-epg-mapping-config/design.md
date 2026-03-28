## Context

Currently, the application parses M3U files and generates M3U files, but the `tvg-id` mapping is not robust enough to handle region-specific EPG data. EPG files can contain multiple entries for the same channel name, differentiated by regions (e.g., `cn`, `hk`, `tw`). We need a mechanism to map M3U channels to the correct EPG `tvg-id` based on a predefined region priority (`zh` -> `hk` -> `tw`). The mapping data will be extracted from an external EPG XML file and stored as a JSON asset.

## Goals / Non-Goals

**Goals:**
- Create a script or utility function to parse an external EPG XML file (`/Users/meow.zang/Desktop/epg.xml`) and generate a JSON mapping file (`src/assets/epg_mapping.json`).
- Define a data structure for the mapping: `[{"name":"xxxx", "channel":"111","source":"cn"}]`.
- Load the JSON mapping file into a global variable (e.g., using `once_cell::sync::Lazy` or `std::sync::OnceLock`) upon server startup.
- Implement a matching algorithm during M3U generation to assign the correct `tvg-id` based on channel name and region priority.

**Non-Goals:**
- Modifying the external EPG XML file itself.
- Creating a UI for managing this mapping (it will be statically generated and loaded).
- Real-time updates to the mapping without restarting the server.

## Decisions

1. **Mapping Storage**:
   - **Decision**: Store the mapping in `src/assets/epg_mapping.json`.
   - **Rationale**: JSON is easy to parse, read, and maintain. Including it in `src/assets` makes it part of the application's static assets.
2. **Global State**:
   - **Decision**: Use `std::sync::OnceLock` (or `once_cell::sync::Lazy`) to load the mapping once at startup.
   - **Rationale**: The mapping is read-only after startup. A global read-only structure avoids the overhead of passing it around or locking it with a `Mutex`/`RwLock`.
3. **Data Structure for Fast Lookup**:
   - **Decision**: While the JSON is an array `[{"name":"xxxx", "channel":"111","source":"cn"}]`, the in-memory representation should be optimized for lookups. A `HashMap<String, Vec<EpgMapping>>` where the key is the channel name will allow O(1) retrieval of all region mappings for a given channel.
4. **Matching Algorithm**:
   - **Decision**:
     1. Lookup by `tv_name` in the `HashMap`.
     2. If found, iterate through the priority list (`zh`, `hk`, `tw`).
     3. If a match for the priority region is found, use its `channel` ID.
     4. If no priority region matches, use the first available mapping.
     5. If `tv_name` is not found, fallback to the channel's display name (the last part of the M3U line) and repeat the lookup.
     6. If still not found, fallback to using the channel name itself as the `tvg-id`.
   - **Rationale**: This provides a robust fallback mechanism ensuring that a `tvg-id` is always assigned, prioritizing the most accurate and preferred region data.

## Risks / Trade-offs

- **Memory Usage**: Loading a large EPG mapping into memory could increase the application's memory footprint.
  - *Mitigation*: The mapping only contains `name`, `channel`, and `source`, which is relatively small compared to full EPG data.
- **Stale Data**: The JSON mapping is generated from an external XML file. If the external file changes, the JSON needs to be regenerated and the server restarted.
  - *Mitigation*: This is acceptable for now as EPG channel IDs do not change frequently. Future iterations could add an API endpoint to reload the mapping.