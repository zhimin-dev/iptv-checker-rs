# epg-export Specification

## Purpose
TBD - created by archiving change epg-info-api. Update Purpose after archive.
## Requirements
### Requirement: EPG Export API
The system SHALL provide an API endpoint `/epg/info/{id}` that generates and returns a custom XMLTV file based on the channel names found in `static/output/{id}.json`.

#### Scenario: Successful EPG Generation
- **WHEN** a client sends a GET request to `/epg/info/{id}` and the corresponding JSON file exists
- **THEN** the system SHALL extract the channel names, query the global EPG cache, generate a valid XMLTV string, and return it with a 200 OK status and `application/xml` content type.

#### Scenario: Missing Output File
- **WHEN** a client sends a GET request to `/epg/info/{id}` but `static/output/{id}.json` does not exist
- **THEN** the system SHALL return a 404 Not Found status.

#### Scenario: Invalid Output File Format
- **WHEN** a client sends a GET request to `/epg/info/{id}` but the JSON file cannot be parsed
- **THEN** the system SHALL return a 500 Internal Server Error or 400 Bad Request status.

