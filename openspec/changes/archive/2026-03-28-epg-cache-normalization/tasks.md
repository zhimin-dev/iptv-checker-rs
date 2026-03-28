## 1. EPG Channel Normalization

- [x] 1.1 Locate the EPG XML parsing logic in `src/epg_xml.rs` (likely within `parse_epg_xml` or similar function that populates `GLOBAL_EPG_CACHE`).
- [x] 1.2 Import `crate::common::translate::trad_to_simp` and `crate::epg_mapping::get_best_tvg_id`.
- [x] 1.3 Update the `<channel>` parsing block to convert the extracted `display-name` to Simplified Chinese using `trad_to_simp`.
- [x] 1.4 Update the `<channel>` parsing block to map the original channel ID to a standardized ID using `get_best_tvg_id(None, &simplified_name)`.
- [x] 1.5 Create a temporary mapping (`HashMap<String, String>`) to store the relationship between the original XML channel ID and the newly mapped standardized ID.

## 2. EPG Programme Normalization

- [x] 2.1 Update the `<programme>` parsing block to intercept the `channel` attribute.
- [x] 2.2 Use the temporary mapping created in step 1.5 to replace the program's original channel ID with the standardized ID.
- [x] 2.3 Ensure the normalized channel and programme data are correctly inserted into `GLOBAL_EPG_CACHE`.