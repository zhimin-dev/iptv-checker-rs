## ADDED Requirements

### Requirement: EPG Channel List API
The system SHALL provide REST API endpoints to list all available channels in the EPG cache.

#### Scenario: Listing all available EPG channels
- **WHEN** a client sends a `GET /api/epg/channel-list` request
- **THEN** the system returns a JSON object containing a `list` array.
- **AND** each item in the array contains `name` (the channel name) and `channel` (the original channel ID from the XML).
