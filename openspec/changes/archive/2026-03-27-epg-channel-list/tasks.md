## 1. EPG Channel List Helper

- [x] 1.1 In `src/epg_xml.rs`, define a new struct `EpgChannelItem` with `name: String` and `channel: String`.
- [x] 1.2 In `src/epg_xml.rs`, implement a function `get_all_epg_channels() -> Vec<EpgChannelItem>` that reads `GLOBAL_EPG_CACHE`.
- [x] 1.3 In the function, iterate over the cache keys (channel names). For each key, get the first `Programme` in the list to extract its `channel` ID, and push it to the result vector.

## 2. EPG Channel List API

- [x] 2.1 In `src/web.rs`, define a response struct `EpgChannelListResponse` with `list: Vec<EpgChannelItem>`.
- [x] 2.2 Create a new Actix-Web handler `get_epg_channel_list` for `GET /epg/channel-list`.
- [x] 2.3 In the handler, call `get_all_epg_channels()` and return it wrapped in `EpgChannelListResponse`.
- [x] 2.4 Register the new service in the `App::new()` builder in `start_web`.
