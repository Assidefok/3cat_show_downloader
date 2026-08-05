//! Fetches the episode list for a TV show from the 3cat API.

use std::sync::Arc;

use tracing::instrument;

use super::api_structs;
use crate::error::{Error, Result};
use crate::http_client::HttpClientTrait;
use crate::models::MediaItem;

const TV3_EPISODE_LIST_URL: &str = "https://www.3cat.cat/api/3cat/dades/?queryKey=%5B%22tira%22%2C%7B%22url%22%3A%22%2F%2Fapi.3cat.cat%2Fvideos%3F_format%3Djson%26no_agrupacio%3DPUAGR_LLSIGN%26tipus_contingut%3DPPD%26items_pagina%3D1500%26pagina%3D1%26sdom%3Dimg%26version%3D2.0%26cache%3D180%26https%3Dtrue%26master%3Dyes%26programatv_id%3D{tv_show_id}%26origen%3Dauto%26perfil%3Dpc%22%7D%5D";

#[instrument(skip(http_client))]
pub(crate) async fn get_episodes<T>(http_client: &Arc<T>, tv_show_id: i32) -> Result<Vec<MediaItem>>
where
    T: HttpClientTrait,
{
    let mut episodes: Vec<MediaItem> = vec![];

    let url = TV3_EPISODE_LIST_URL.replace("{tv_show_id}", &tv_show_id.to_string());

    let tv3_tv_show_api_response = http_client
        .get::<api_structs::EpisodesRoot, api_structs::Tv3Error>(&url, None)
        .await
        .map_err(|e| Error::Decoding {
            context: format!("episode list url={url}"),
            source: Box::new(e),
        })?;

    let episode_list = tv3_tv_show_api_response.response.items.item;
    if episode_list.is_empty() {
        return Ok(episodes);
    }

    for item in episode_list {
        // Skip items missing the minimum data needed to build a MediaItem.
        // The 3cat API returns heterogeneous items (e.g., trailers lacking
        // `programa` or `permatitle`); these would otherwise produce invalid
        // MediaItems downstream.
        let (Some(id), Some(perma)) = (item.id, item.permatitle.as_ref()) else {
            continue;
        };

        let title = match item.title {
            Some(t) if !t.is_empty() => t,
            _ => perma.clone(),
        };

        episodes.push(MediaItem {
            id,
            title,
            video_url: None,
            subtitle_url: None,
            episode_number: item.number_of_episode,
            tv_show_name: item.tv_show_name,
            season: None,
            subtitle_failed: false,
        });
    }

    Ok(episodes)
}
