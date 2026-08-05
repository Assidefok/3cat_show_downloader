//! Media item model, subtitle handling mode, download parameters, and filename generation.

use std::sync::Arc;

use indicatif::MultiProgress;
use regex::Regex;
use unidecode::unidecode;

use crate::error::Result;
use crate::http_client::HttpClient;

/// Controls how subtitles are handled during media downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleMode {
    /// Do not download subtitles at all.
    Skip,
    /// Download subtitles as separate `.vtt` files.
    Download,
    /// Download subtitles and embed them into the video file via ffmpeg.
    Embed,
}

/// Common parameters shared across media download operations.
///
/// All fields are cheaply cloneable so the struct can be shared across
/// spawned Tokio tasks without lifetime constraints.
#[derive(Clone, Debug)]
pub struct DownloadParams {
    /// Shared HTTP client instance.
    pub http_client: Arc<HttpClient>,
    /// How subtitles should be handled during downloads.
    pub subtitle_mode: SubtitleMode,
    /// Number of concurrent download tasks (1-10).
    pub concurrent_downloads: u8,
    /// Shared multi-progress bar renderer.
    pub multi_progress: MultiProgress,
    /// Output directory for downloaded files.
    pub directory: Arc<str>,
    /// Whether `yt-dlp` is available on the system PATH.
    ///
    /// When `true`, yt-dlp is used as the download backend instead of the
    /// built-in HTTP downloader. yt-dlp handles format selection and subtitle
    /// extraction internally without a prior API call.
    pub yt_dlp_available: bool,
    /// When `true`, subtitle download / clean / embed failures abort the
    /// entire batch (the legacy fail-fast behaviour). When `false` (default),
    /// subtitle failures are logged as warnings and the video is still saved
    /// — see `--strict-subtitles` in the CLI.
    pub strict_subtitles: bool,
    /// When `true`, the file is saved using the
    /// `Mic - <episode_slug> - S<season>E<ep> (<format>)` convention and
    /// organized into `Temporada XX/` (or `Pel·lícules/` for movies)
    /// subdirectories under the user-supplied output directory.
    /// See `--auto-naming` in the CLI.
    pub auto_naming: bool,
}

/// Represents a downloadable media item (TV show episode or movie).
#[derive(Debug)]
pub struct MediaItem {
    /// Internal 3cat media ID.
    pub id: i32,
    /// Title of the media item.
    pub title: String,
    /// URL to the video file, populated after fetching media details.
    pub video_url: Option<String>,
    /// URL to the subtitle file, populated after fetching media details.
    pub subtitle_url: Option<String>,
    /// Sequential episode number within the show (`None` for movies).
    pub episode_number: Option<i32>,
    /// Name of the TV show this episode belongs to (`None` for movies).
    pub tv_show_name: Option<String>,
    /// Season number used when auto-naming is enabled. `None` for movies and
    /// when the source API does not expose it. When supplied, the value is
    /// zero-padded to two digits in the generated filename.
    pub season: Option<i32>,
    /// Set to `true` when the subtitle pipeline (download / clean / embed)
    /// failed for this item and the video was saved without subtitles.
    /// Reported in the final run summary so the user can decide whether to
    /// retry with `--skip-subtitles` or `--strict-subtitles`.
    pub subtitle_failed: bool,
}

impl MediaItem {
    /// Generates a sanitized filename for the media item with the given extension.
    ///
    /// Behaviour depends on [`crate::models::DownloadParams::auto_naming`]:
    ///
    /// * **Auto-naming enabled**: produces
    ///   `Mic - <slug> - S<season>E<episode> (<ext>)` for episodes that have
    ///   both `episode_number` and `season` set, and
    ///   `Mic - <slug> (<ext>)` for movies. `<season>` and `<episode>` are
    ///   zero-padded to two digits.
    /// * **Auto-naming disabled** (default): preserves the legacy format —
    ///   `<episode>-<slug>.<ext>` for episodes (with an `ova-` prefix when
    ///   the show name contains "OVA"), or `<slug>.<ext>` for movies.
    ///
    /// # Errors
    ///
    /// Returns an error if the internal regex patterns fail to compile.
    pub fn filename(&self, extension: &str, auto_naming: bool) -> Result<String> {
        let slug = Self::slugify(&self.title)?;

        if auto_naming {
            return Ok(match (self.episode_number, self.season) {
                (Some(ep), Some(season)) => format!(
                    "Mic - {slug} - S{season:02}E{ep:02} ({extension})"
                ),
                (Some(ep), None) => format!("Mic - {slug} - S01E{ep:02} ({extension})"),
                (None, _) => format!("Mic - {slug} ({extension})"),
            });
        }

        match (self.episode_number, &self.tv_show_name) {
            (Some(ep_num), Some(show_name)) if show_name.to_lowercase().contains("ova") => {
                Ok(format!("ova-{ep_num}-{slug}.{extension}"))
            }
            (Some(ep_num), _) => Ok(format!("{ep_num}-{slug}.{extension}")),
            (None, _) => Ok(format!("{slug}.{extension}")),
        }
    }

    /// Returns the relative subdirectory under the user-supplied output
    /// directory where this item's files should be saved, when auto-naming
    /// is enabled. Returns `None` for the flat (legacy) layout.
    ///
    /// Layout rules:
    ///
    /// * **Episode with season**: `Temporada XX/`.
    /// * **Movie**: `Pel·lícules/`.
    /// * **Episode without season** but with episode_number: `Temporada 01/`.
    /// * **Item with no episode_number and no season** (treated as movie): `Pel·lícules/`.
    pub fn subdirectory(&self, auto_naming: bool) -> Option<String> {
        if !auto_naming {
            return None;
        }
        if self.episode_number.is_some() {
            let season = self.season.unwrap_or(1);
            Some(format!("Temporada {season:02}"))
        } else {
            Some("Pel·lícules".to_string())
        }
    }

    /// Converts a title into a URL-friendly slug.
    fn slugify(title: &str) -> Result<String> {
        let lowercased = title.to_lowercase();
        let unaccented = unidecode(&lowercased);
        let re = Regex::new(r"[^a-z0-9\s-]")?;
        let cleaned = re.replace_all(&unaccented, "");
        let dash_replaced = cleaned.replace(' ', "-");
        let collapsed = Regex::new(r"-+")?.replace_all(&dash_replaced, "-");
        Ok(collapsed.trim_matches('-').to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_generate_correct_filename_for_episode() {
        let item = MediaItem {
            id: 1,
            title: "T1xC7 - Veureu una cosa al·lucinant i màgica!".to_string(),
            video_url: None,
            subtitle_url: None,
            episode_number: Some(7),
            tv_show_name: Some("Tv show name".to_string()),
            season: None,
            subtitle_failed: false,
        };
        assert_eq!(
            item.filename("mp4", false).unwrap(),
            "7-t1xc7-veureu-una-cosa-allucinant-i-magica.mp4"
        );
    }

    #[test]
    fn test_should_prefix_ova_in_filename() {
        let item = MediaItem {
            id: 1,
            title: "T1xC7 - Veureu una cosa al·lucinant!".to_string(),
            video_url: None,
            subtitle_url: None,
            episode_number: Some(7),
            tv_show_name: Some("Tv show name (OVA)".to_string()),
            season: None,
            subtitle_failed: false,
        };
        assert_eq!(
            item.filename("mp4", false).unwrap(),
            "ova-7-t1xc7-veureu-una-cosa-allucinant.mp4"
        );
    }

    #[test]
    fn test_should_generate_filename_for_movie() {
        let item = MediaItem {
            id: 42,
            title: "El secret de la cova".to_string(),
            video_url: None,
            subtitle_url: None,
            episode_number: None,
            tv_show_name: None,
            season: None,
            subtitle_failed: false,
        };
        assert_eq!(
            item.filename("mp4", false).unwrap(),
            "el-secret-de-la-cova.mp4"
        );
    }

    #[test]
    fn test_should_generate_auto_naming_filename_for_episode() {
        let item = MediaItem {
            id: 1,
            title: "T1xC7 - Veureu una cosa al·lucinant i màgica!".to_string(),
            video_url: None,
            subtitle_url: None,
            episode_number: Some(7),
            tv_show_name: Some("Bola de Drac".to_string()),
            season: Some(1),
            subtitle_failed: false,
        };
        assert_eq!(
            item.filename("mp4", true).unwrap(),
            "Mic - t1xc7-veureu-una-cosa-allucinant-i-magica - S01E07 (mp4)"
        );
        assert_eq!(
            item.subdirectory(true).as_deref(),
            Some("Temporada 01")
        );
    }

    #[test]
    fn test_should_generate_auto_naming_filename_for_movie() {
        let item = MediaItem {
            id: 42,
            title: "El secret de la cova".to_string(),
            video_url: None,
            subtitle_url: None,
            episode_number: None,
            tv_show_name: None,
            season: None,
            subtitle_failed: false,
        };
        assert_eq!(
            item.filename("mp4", true).unwrap(),
            "Mic - el-secret-de-la-cova (mp4)"
        );
        assert_eq!(
            item.subdirectory(true).as_deref(),
            Some("Pel·lícules")
        );
    }

    #[test]
    fn test_should_default_season_to_01_when_auto_naming_and_no_season() {
        let item = MediaItem {
            id: 1,
            title: "Pilot".to_string(),
            video_url: None,
            subtitle_url: None,
            episode_number: Some(3),
            tv_show_name: Some("Show".to_string()),
            season: None,
            subtitle_failed: false,
        };
        assert_eq!(
            item.filename("mp4", true).unwrap(),
            "Mic - pilot - S01E03 (mp4)"
        );
    }

    #[test]
    fn test_should_zero_pad_season_above_99() {
        let item = MediaItem {
            id: 1,
            title: "Final".to_string(),
            video_url: None,
            subtitle_url: None,
            episode_number: Some(1),
            tv_show_name: Some("Show".to_string()),
            season: Some(123),
            subtitle_failed: false,
        };
        assert_eq!(
            item.filename("mp4", true).unwrap(),
            "Mic - final - S123E01 (mp4)"
        );
    }

    #[test]
    fn test_should_not_create_subdirectory_when_auto_naming_disabled() {
        let item = MediaItem {
            id: 1,
            title: "Pilot".to_string(),
            video_url: None,
            subtitle_url: None,
            episode_number: Some(3),
            tv_show_name: Some("Show".to_string()),
            season: Some(1),
            subtitle_failed: false,
        };
        assert_eq!(item.subdirectory(false), None);
    }
}
