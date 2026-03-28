# epg-data-management Specification

## Purpose
TBD - created by archiving change epg-sources-status. Update Purpose after archive.
## Requirements
### Requirement: EPG Source Management API
The system SHALL provide REST API endpoints to list, add, and remove EPG source URLs. The list endpoint MUST also include a status indicating if today's EPG data has been downloaded. The system SHALL also provide an endpoint to delete the current day's EPG cache.

#### Scenario: Listing EPG sources when today's data exists
- **WHEN** a client sends a `GET /api/epg/sources` request
- **AND** the directory `static/epg/{YYYY-MM-DD}` (for today's date) exists
- **THEN** the system returns a JSON object with `list` containing all configured EPG URLs and `status` set to `true`.

#### Scenario: Listing EPG sources when today's data does not exist
- **WHEN** a client sends a `GET /api/epg/sources` request
- **AND** the directory `static/epg/{YYYY-MM-DD}` (for today's date) does NOT exist
- **THEN** the system returns a JSON object with `list` containing all configured EPG URLs and `status` set to `false`.

#### Scenario: Deleting the EPG cache successfully
- **WHEN** a client sends a `DELETE /api/epg/cache` request
- **AND** the directory `static/epg/{YYYY-MM-DD}` exists
- **THEN** the system deletes the directory and returns a success JSON response.

#### Scenario: Deleting the EPG cache when it does not exist
- **WHEN** a client sends a `DELETE /api/epg/cache` request
- **AND** the directory `static/epg/{YYYY-MM-DD}` does NOT exist
- **THEN** the system returns a success JSON response (idempotent behavior).

### Requirement: EPG Channel List API
The system SHALL provide REST API endpoints to list all available channels in the EPG cache.

#### Scenario: Listing all available EPG channels
- **WHEN** a client sends a `GET /api/epg/channel-list` request
- **THEN** the system returns a JSON object containing a `list` array.
- **AND** each item in the array contains `name` (the channel name) and `channel` (the original channel ID from the XML).

