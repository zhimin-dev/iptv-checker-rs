## 1. EPG XML Generation Logic

- [x] 1.1 Add `generate_custom_epg_xml` function in `src/epg_xml.rs` that takes a list of channel names, queries `GLOBAL_EPG_CACHE`, constructs a `Tv` object, and converts it to XML string using `tv_to_epg_xml`.

## 2. API Endpoint Implementation

- [x] 2.1 Add `get_epg_info` handler in `src/web.rs` for `GET /epg/info/{id}`.
- [x] 2.2 Implement logic to read `static/output/{id}.json` in the handler.
- [x] 2.3 Parse the JSON file to extract `tv_name` (or fallback to `name`) from the `list` array.
- [x] 2.4 Call `generate_custom_epg_xml` with the extracted channel names.
- [x] 2.5 Return the generated XML with `Content-Type: application/xml`.
- [x] 2.6 Register the new endpoint in `start_web` function in `src/web.rs`.
