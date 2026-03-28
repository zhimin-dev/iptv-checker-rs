## Context

Currently, the application caches EPG data globally, but users lack a way to generate a custom XMLTV file tailored to their specific playlist output. A playlist output is generated as a JSON file (e.g., `1111.json`) in `static/output/`. This feature will read that JSON file, extract the channel names, and generate a valid XMLTV string containing only the programs for those channels.

## Goals / Non-Goals

**Goals:**
- Provide an API endpoint `GET /epg/info/{id}` that returns a custom XMLTV file.
- Read and parse the `static/output/{id}.json` file.
- Extract unique channel names from the JSON output.
- Query the global EPG cache for the extracted channels.
- Generate and return the XMLTV string with the correct content type.

**Non-Goals:**
- Generating EPG data for channels not present in the global cache.
- Modifying the existing playlist JSON output format.

## Decisions

1. **API Endpoint**: `GET /epg/info/{id}`. This is simple and RESTful. The `id` corresponds to the filename `{id}.json`.
2. **JSON Parsing**: The `static/output/{id}.json` file contains an `M3uObjectList` structure. We will parse it into a `serde_json::Value` or directly deserialize it into an `M3uObjectList` if possible, to extract `list[].extend.tv_name` (or fallback to `list[].name`).
3. **EPG Generation**: We will create a new function `generate_custom_epg_xml(channel_names: Vec<String>) -> String` in `src/epg_xml.rs` that queries the `GLOBAL_EPG_CACHE`, constructs a `Tv` object with the relevant channels and programmes, and calls `tv_to_epg_xml`.
4. **Response Format**: The API will return `HttpResponse::Ok().content_type("application/xml").body(xml_string)`.

## Risks / Trade-offs

- **Risk**: The JSON file might not exist or might be malformed.
  - **Mitigation**: Handle file reading and parsing errors gracefully, returning a 404 or 400 error response.
- **Risk**: The generated XML string might be very large.
  - **Mitigation**: The XML generation is done in memory. If memory becomes an issue, we might need to stream the XML response, but for typical IPTV playlists, memory generation should be sufficient.
