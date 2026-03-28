## ADDED Requirements

### Requirement: EPG Source Management API
The system SHALL provide REST API endpoints to list, add, and remove EPG source URLs. The list endpoint MUST also include a status indicating if today's EPG data has been downloaded.

#### Scenario: Listing EPG sources when today's data exists
- **WHEN** a client sends a `GET /api/epg/sources` request
- **AND** the directory `static/epg/{YYYY-MM-DD}` (for today's date) exists
- **THEN** the system returns a JSON object with `list` containing all configured EPG URLs and `status` set to `true`.

#### Scenario: Listing EPG sources when today's data does not exist
- **WHEN** a client sends a `GET /api/epg/sources` request
- **AND** the directory `static/epg/{YYYY-MM-DD}` (for today's date) does NOT exist
- **THEN** the system returns a JSON object with `list` containing all configured EPG URLs and `status` set to `false`.
