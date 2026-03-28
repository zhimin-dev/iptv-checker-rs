## Why

Currently, the EPG data is downloaded and cached in the `static/epg/{YYYY-MM-DD}` directory. However, there is no way for the user or frontend to manually clear this cache if the data is corrupted, outdated, or needs to be forcefully re-downloaded. Providing an API to delete today's EPG cache folder solves this problem and gives users more control over their EPG data.

## What Changes

- Add a new REST API endpoint `DELETE /api/epg/cache` (or similar).
- The endpoint will calculate today's date (`%Y%m%d` or `%Y-%m-%d` depending on the current implementation), locate the corresponding `static/epg/{date}` folder, and delete it along with its contents.
- Return a success status if the folder was deleted or didn't exist, and an error if the deletion failed.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `epg-data-management`: Adding a new requirement for an API endpoint to delete the current day's EPG cache directory.

## Impact

- **New API Endpoint**: A new route will be added to `src/web.rs`.
- **File System**: The application will perform recursive directory deletion (`std::fs::remove_dir_all`).
