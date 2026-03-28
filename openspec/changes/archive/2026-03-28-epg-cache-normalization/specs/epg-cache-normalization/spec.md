## ADDED Requirements

### Requirement: EPG Data Normalization
The system SHALL normalize EPG channel data before storing it in the global cache. Normalization MUST include converting the channel's `display-name` from Traditional to Simplified Chinese, and mapping the channel's ID to a standardized `tvg-id` using the `epg_mapping` logic based on the simplified name.

#### Scenario: Caching EPG data with Traditional Chinese names
- **WHEN** the system parses an EPG XML file containing a channel with a Traditional Chinese `display-name` (e.g., "鳳凰衛视")
- **THEN** it converts the `display-name` to Simplified Chinese (e.g., "凤凰卫视") before storing it in the cache.

#### Scenario: Caching EPG data with regional channel IDs
- **WHEN** the system parses an EPG XML file containing a channel with a regional ID (e.g., "CCTV-1.hk")
- **THEN** it uses the channel's simplified display name to look up the standardized ID via the `epg_mapping` priority list
- **AND** replaces the original regional ID with the standardized ID before storing it in the cache.

### Requirement: EPG Programme Normalization
The system SHALL ensure that all `programme` entries in the EPG cache reference the newly mapped standardized channel IDs instead of the original raw channel IDs.

#### Scenario: Linking programs to mapped channels
- **WHEN** the system parses a `<programme>` tag with a `channel` attribute pointing to a regional ID (e.g., "CCTV-1.hk")
- **THEN** it replaces the `channel` attribute with the corresponding standardized ID (e.g., "CCTV-1.cn") that was determined during channel normalization
- **AND** stores the updated `programme` in the cache under the standardized ID.
