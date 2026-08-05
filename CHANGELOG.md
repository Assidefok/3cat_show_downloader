## Unreleased

- Subtitle downloads are now tolerant: when the 3cat API is missing a subtitle URL, the subtitle stream fails to download, or `subtitle_cleaner` fails to read/clean the VTT for a given episode, the failure is logged as a `tracing::warn!` and the video is still saved. The previously failing episode can be re-downloaded with `--skip-subtitles` to silence the warning. Pass `--strict-subtitles` to restore the legacy fail-fast behaviour (any subtitle error aborts the whole batch).
- Add `--auto-naming` (and optional `--season N`, default 1): when enabled, files are saved as `Mic - <episode_slug> - S<season:02>E<ep:02> (<ext>)` and organised under `Temporada XX/` (episodes) or `Pel·lícules/` (movies) subdirectories inside the user-supplied output directory. The auto-naming subdirectory is created on demand.
- Fix `decoding error: request body read error: error decoding response body` when fetching TV show episode lists and single-episode metadata from the 3cat API. The endpoint returns heterogeneous items where some entries omit `programa` (and a few other consumed fields); making every consumed field `Option<T>` with `#[serde(default)]` lets the decoder tolerate the missing fields instead of failing the whole 339-item response on the first absence. Items still missing the minimum data needed to build a `MediaItem` (id + permatitle) are now skipped. The single-episode endpoint is parsed as `serde_json::Value` and its `media.url` is walked defensively because it can be either a single object or an array of objects.
- Preserve the decoding error chain end-to-end: the previous `Error::Decoding(String)` discarded the underlying reqwest/serde_json source. The variant now carries a `context` string (URL + operation) and a boxed `#[source]` error so the chain survives into logs and the CLI error message.
- Extract `HttpClient::format_response` body decoding into a pure `decode_body(status, bytes)` helper and add 7 regression tests covering: valid payload, optional-field absence (mirrors `missing field 'programa'`), empty body, HTML body, schema drift, non-2xx with parseable error, and non-2xx with unparseable body.

## 1.2.0 - 2026-03-19

- Add yt-dlp as an optional download backend: when `yt-dlp` is found on PATH it is used instead of the built-in HTTP downloader. The CCMA/3cat extractor built into yt-dlp handles format selection and subtitle extraction internally, so no prior API call is made. Subtitle embedding (`SubtitleMode::Embed`) forces Matroska output (`--merge-output-format mkv`) for consistency with the ffmpeg path. If yt-dlp is not installed the existing behaviour is unchanged.

## 1.1.0 - 2026-03-18

- Move all module declarations, orchestration logic, and progress writer infrastructure from `main.rs` to `lib.rs`, reducing the binary entry point to a thin ~20-line wrapper that only handles tracing setup and runtime construction.

## 1.0.1 - 2026-03-13

- Handle episodes without subtitles gracefully: the `subtitles` field in the API response is now optional. When subtitles are unavailable and the user has not passed `--skip-subtitles`, a clear error is returned naming the episode and suggesting the flag.

## 1.0.0 - 2026-03-13

- Add movie download support: the tool now automatically detects whether a slug is a TV show or a movie and downloads accordingly.
- Change the slug from a named parameter (`--tv-show-slug` / `-t`) to a positional argument for simpler invocation (e.g. `./cat_show_downloader bola-de-drac -d ./output/`).
- Rename internal `Episode` model to `MediaItem` to support both TV show episodes and movies through a unified download pipeline.
- Restructure `tv_show` and `movie` modules into their own directories with dedicated `api_structs` submodules.

## 0.1.0

- Add parallel downloading support
- Embed subtitles to downloaded files and create MKV files using ffmpeg
- Add progress bar support

## 0.0.2 - 2024-12-02

- Make id retrieval more robust

## 0.0.1 - 2024-12-02

- First release
