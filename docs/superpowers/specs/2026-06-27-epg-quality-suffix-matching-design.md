# EPG Quality Suffix Matching

## Problem

When converting TXT to M3U, channel names often include quality/resolution suffixes like
`东方卫视4k` or `CCTV1_4M1080`. The EPG mapping only has the base name (`东方卫视`,
`CCTV1`), so the exact-match lookup in `get_best_tvg_id` fails, leaving `tvg-name` and
`tvg-id` empty.

## Goal

Match channel names that carry quality suffixes against the EPG mapping, so that correct
`tvg-name` and `tvg-id` are set from EPG data. The original display name (with quality
info) is preserved; only the EPG metadata fields use the clean base name.

## Design

### Algorithm (`epg_mapping.rs`)

Add a new function that returns both the EPG `name` and `channel` (the existing
`get_best_tvg_id` stays as-is for the output-phase fallback):

```
fn match_epg_channel(raw_name: &str) -> Option<(String, String)>
    // returns Some((epg_name, epg_channel_id)) or None
```

Three-tier matching, tried in order:

1. **Exact match** (fast path, no change for channels without suffixes):
   `东方卫视` → exact hit → `("东方卫视", "xxx")`

2. **Strip known quality patterns, exact match**:
   Strip quality suffixes anchored to the end of the name, then exact match against EPG.
   - Resolution tags: `4K`, `8K`, `2K`, `1080P`, `720P`, `480P`, `360P`, `240P`, `HD`, `FHD`, `UHD`, `SD`, `HEVC`, `HDR` (case-insensitive, optional leading space)
   - Bitrate patterns: `_N_M`, `_N_Mxxxx` (e.g. `_4M`, `_4M1080`, `_8M720`)
   - Stripped text may appear with optional leading space, wrapping brackets/parens, or
     underscore prefix
   - Only strips from the **end** of the name, so legitimate channel brands like
     `Clarity4K` are not affected
   - `东方卫视4k` → strip `4k` → `东方卫视` → hit
   - `CCTV1_4M1080` → strip `_4M1080` → `CCTV1` → hit

3. **Longest-prefix fallback**:
   Find the longest EPG key that is a prefix of the channel name. Handles novel suffix
   patterns without regex maintenance.
   - `东方卫视超清` — no regex match in tier 2, but `东方卫视` is a prefix → hit
   - No match if no EPG entry qualifies as a prefix

Within each tier, the existing source priority applies: zh > cn > hk > tw.

### Integration Points

**Parsing** (`parse_quota_str` in `util.rs`, `parse_normal_str` in `util.rs`):
After building the `M3uObject`, call `match_epg_channel` with the raw channel name. If
matched, set `extend.tv_name` and `extend.tv_id` at parse time.

**Output** (`generate_raw` in `m3u.rs`):
Existing logic already writes `tvg-name` from `extend.tv_name` and calls
`get_best_tvg_id` for `tvg-id`. No changes needed — the clean EPG name and ID are
already populated from parsing. `get_best_tvg_id` acts as a secondary fallback if
`tv_name` somehow wasn't set at parse time.

**Display name** (`M3uObject.name`):
Unchanged — keeps the original name with quality suffix. The M3U display line shows
`东方卫视4k`, but `tvg-name="东方卫视"`.

**Backward compatibility**:
If no EPG match, `tv_name` stays empty and the existing fallback in `generate_raw` uses
the display name as-is.

## Testing

- Channel without suffix: `东方卫视` → exact EPG match, no change
- Channel with simple suffix: `东方卫视4k` → tier 2 match
- Channel with bitrate: `CCTV1_4M1080` → tier 2 match
- Channel with bracket: `湖南卫视[4K]` → tier 2 match
- Channel with unknown suffix: `东方卫视超清` → tier 3 prefix match
- Legitimate quality brand: `Clarity4K Фантастика` → exact match (tier 1), not stripped
- No EPG entry exists: `UnknownChannel123 4K` → no match, defaults preserved
