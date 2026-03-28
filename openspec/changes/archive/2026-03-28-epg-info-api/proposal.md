## Why

Users who have checked their IPTV playlists and generated an output file (e.g., `1111.json`) need a way to get the corresponding EPG data specifically for the channels in that playlist. Currently, they can query individual channels, but generating a full XMLTV file tailored to their specific playlist output is missing. This feature allows users to easily import a customized EPG XML file into their IPTV players.

## What Changes

- Add a new REST API endpoint `GET /api/epg/info/{id}` (or `GET /epg/info/{id}`).
- The endpoint will read the local JSON file at `static/output/{id}.json`.
- It will extract all the channel names (`tv_name` or `name`) from the `list` array in the JSON.
- It will query the global EPG cache (`GLOBAL_EPG_CACHE`) for these specific channels.
- It will construct a new `Tv` object containing only the matched channels and their programs.
- It will serialize this `Tv` object into an XMLTV string using the existing `tv_to_epg_xml` function.
- It will return the generated XML string with the `Content-Type: application/xml` header.

## Capabilities

### New Capabilities
- `epg-export`: A new capability to generate and export customized XMLTV EPG data based on a specific playlist output ID.

### Modified Capabilities


## Impact

- **New API Endpoint**: A new route will be added to `src/web.rs`.
- **File System**: The application will read from `static/output/{id}.json`.
- **EPG Module**: A new helper function might be added to `src/epg_xml.rs` to construct a filtered `Tv` object from a list of channel names.
