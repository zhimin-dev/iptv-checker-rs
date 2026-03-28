## 1. Modify GET /epg/sources API

- [x] 1.1 Define a new struct `EpgSourcesResponse` with `list: Vec<String>` and `status: bool` in `src/web.rs`.
- [x] 1.2 Update the `get_epg_sources` handler to get today's date using `chrono::Local::now().format("%Y-%m-%d")`.
- [x] 1.3 Construct the path `static/epg/{date}` and check if it exists using `std::path::Path::new(&path).exists()`.
- [x] 1.4 Return the `EpgSourcesResponse` as JSON instead of the raw list.
