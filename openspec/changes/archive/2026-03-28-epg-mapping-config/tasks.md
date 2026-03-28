## 1. EPG Mapping Generation

- [x] 1.1 Create a script or utility function to parse the external EPG XML file (`/Users/meow.zang/Desktop/epg.xml`).
- [x] 1.2 Extract `name`, `channel` ID, and `source` region from the XML.
- [x] 1.3 Format the extracted data into a JSON array of objects (`[{"name":"xxxx", "channel":"111","source":"cn"}]`).
- [x] 1.4 Save the JSON output to `src/assets/epg_mapping.json`.

## 2. Global State Setup

- [x] 2.1 Define the Rust struct `EpgMapping` corresponding to the JSON structure.
- [x] 2.2 Create a global variable (e.g., using `std::sync::OnceLock` or `once_cell::sync::Lazy`) to hold the mapping data as a `HashMap<String, Vec<EpgMapping>>`.
- [x] 2.3 Implement the initialization logic to read `src/assets/epg_mapping.json` and populate the global `HashMap` upon server startup.

## 3. M3U Generation Update

- [x] 3.1 Locate the M3U generation logic where `tvg-id` is assigned.
- [x] 3.2 Implement the matching algorithm: lookup by `tv_name` in the global `HashMap`.
- [x] 3.3 Implement the region priority filtering (`zh` -> `hk` -> `tw`) for matched results.
- [x] 3.4 Implement fallback logic: if `tv_name` fails, lookup by channel display name.
- [x] 3.5 Implement final fallback: if no match is found, assign the channel display name to `tvg-id`.
- [x] 3.6 Update the M3U writer to output the assigned `tvg-id`.
