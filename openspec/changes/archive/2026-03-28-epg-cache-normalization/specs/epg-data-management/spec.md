## MODIFIED Requirements

### Requirement: EPG Channel List API
The system SHALL provide REST API endpoints to list all available channels in the EPG cache. The returned channels MUST reflect the normalized data (Simplified Chinese names and standardized channel IDs).

#### Scenario: Listing all available EPG channels
- **WHEN** a client sends a `GET /api/epg/channel-list` request
- **THEN** the system returns a JSON object containing a `list` array.
- **AND** each item in the array contains `name` (the normalized Simplified Chinese channel name) and `channel` (the standardized channel ID mapped via `epg_mapping`).
