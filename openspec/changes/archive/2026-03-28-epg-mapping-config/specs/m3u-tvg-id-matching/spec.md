## ADDED Requirements

### Requirement: M3U `tvg-id` Matching Algorithm
The system SHALL assign a `tvg-id` to channels during M3U generation based on the global EPG mapping. The matching algorithm MUST prioritize the `tv_name` attribute, filter by a predefined region priority (`zh` -> `hk` -> `tw`), and provide fallback mechanisms.

#### Scenario: Exact match with region priority
- **WHEN** generating an M3U entry for a channel with `tv_name` "CCTV-1"
- **AND** the global mapping contains mappings for "CCTV-1" with sources `cn`, `hk`, and `tw`
- **THEN** the system selects the mapping with source `zh` (or `cn` if equivalent) based on the priority list
- **AND** assigns the corresponding `channel` ID to the `tvg-id` attribute

#### Scenario: Fallback to lower priority region
- **WHEN** generating an M3U entry for a channel with `tv_name` "TVB"
- **AND** the global mapping contains mappings for "TVB" with sources `hk` and `tw` only
- **THEN** the system selects the mapping with source `hk` based on the priority list
- **AND** assigns the corresponding `channel` ID to the `tvg-id` attribute

#### Scenario: Fallback to first available mapping
- **WHEN** generating an M3U entry for a channel with `tv_name` "BBC"
- **AND** the global mapping contains mappings for "BBC" with source `en` only
- **THEN** the system selects the first available mapping (source `en`) since no priority regions match
- **AND** assigns the corresponding `channel` ID to the `tvg-id` attribute

#### Scenario: Fallback to display name
- **WHEN** generating an M3U entry for a channel that lacks a `tv_name` attribute
- **AND** the channel's display name (last part of the M3U line) is "Phoenix TV"
- **THEN** the system uses "Phoenix TV" to perform the lookup in the global mapping
- **AND** applies the region priority logic as described above

#### Scenario: Fallback to channel name as ID
- **WHEN** generating an M3U entry for a channel
- **AND** neither `tv_name` nor the display name yields a match in the global mapping
- **THEN** the system assigns the channel's display name directly to the `tvg-id` attribute
