## 1. Delete Cache API

- [x] 1.1 Create a new Actix-Web handler `delete_epg_cache_api` for `DELETE /epg/cache` in `src/web.rs`.
- [x] 1.2 In the handler, calculate today's date and construct the path `static/epg/{YYYY-MM-DD}`.
- [x] 1.3 Use `std::fs::remove_dir_all` to delete the directory. Handle `NotFound` errors gracefully by treating them as success.
- [x] 1.4 Register the new service in the `App::new()` builder in `start_web`.
