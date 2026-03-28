## Why

Currently, when generating M3U files, the `tvg-id` is often missing or inaccurate because M3U playlists lack region-specific information for channels. A single channel name might have different EPG data for different regions (e.g., Mainland China, Hong Kong, Taiwan). We need a way to accurately map channel names to their corresponding `tvg-id` based on a predefined region priority (`zh` -> `hk` -> `tw`) to ensure users get the most relevant EPG data for their region.

## What Changes

- Parse an external EPG XML file (`/Users/meow.zang/Desktop/epg.xml`) to extract channel mappings.
- Generate a JSON configuration file (`src/assets/epg_mapping.json`) containing the mapping: `[{"name":"xxxx", "channel":"111","source":"cn"}]`.
- Load this JSON configuration into a global state variable when the server starts.
- Update the M3U generation logic to use this global mapping for assigning `tvg-id`.
- Implement a matching algorithm that prioritizes `tv_name`, falls back to the channel display name, and filters by region priority (`zh` -> `hk` -> `tw`).
- If no region matches the priority list, fallback to the channel name itself as the `tvg-id`.

## Capabilities

### New Capabilities
- `epg-mapping`: Defines the structure and generation of the EPG mapping JSON from an external XML file, and its loading into global state.
- `m3u-tvg-id-matching`: Defines the algorithm for matching M3U channels to EPG `tvg-id`s using the loaded mapping and region priorities.

### Modified Capabilities
<!-- Existing capabilities whose REQUIREMENTS are changing (not just implementation).
     Only list here if spec-level behavior changes. Each needs a delta spec file.
     Use existing spec names from openspec/specs/. Leave empty if no requirement changes. -->


## Impact

- **Startup Process**: The server startup will now include loading the `epg_mapping.json` file into memory.
- **M3U Generation**: The logic for writing M3U files will be modified to perform lookups against the global mapping.
- **File System**: A new asset file (`src/assets/epg_mapping.json`) will be created and maintained.
- **Memory**: A new global variable (e.g., using `once_cell` or `lazy_static`) will be introduced to hold the mapping data in memory.