## ADDED Requirements

### Requirement: EPG Mapping Configuration Generation
The system SHALL provide a mechanism to parse an external EPG XML file and generate a JSON configuration file containing channel mappings. The JSON file MUST be saved to `src/assets/epg_mapping.json` and MUST follow the format `[{"name":"xxxx", "channel":"111","source":"cn"}]`.

#### Scenario: Generate mapping from valid XML
- **WHEN** the generation script or function is executed with a valid external EPG XML file
- **THEN** it parses the XML, extracts the channel name, ID, and source region
- **AND** writes the output to `src/assets/epg_mapping.json` in the specified JSON array format

### Requirement: EPG Mapping Global State Loading
The system SHALL load the `src/assets/epg_mapping.json` file into a global, read-only memory structure upon server startup.

#### Scenario: Server starts with mapping file present
- **WHEN** the server application starts
- **AND** `src/assets/epg_mapping.json` exists
- **THEN** the system reads the file and populates a global mapping structure (e.g., `HashMap`)
- **AND** the server continues startup successfully

#### Scenario: Server starts with mapping file missing
- **WHEN** the server application starts
- **AND** `src/assets/epg_mapping.json` does not exist
- **THEN** the system logs a warning or error
- **AND** initializes an empty global mapping structure to prevent runtime panics
