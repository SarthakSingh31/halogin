use std::collections::HashMap;

use axum::Json;
use diesel::pg::Pg;
use diesel_async::AsyncConnection;
use serde::Deserialize;

use crate::{
    models::{GoogleAccount, User},
    Error, POOL,
};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: String,
    pub snippet: ChannelSnippet,
    pub statistics: ChannelStatistics,
}

impl Channel {
    pub async fn get_for_user_handler(user: User) -> Result<Json<Vec<Self>>, Error> {
        let mut conn = POOL.get().await?;
        Self::get_for_user(user, &mut conn)
            .await
            .map(|channels| Json(channels))
    }

    pub async fn get_for_user(
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Self>, Error> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            page_info: PageInfo,
            items: Vec<Channel>,
        }

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PageInfo {
            total_results: usize,
            results_per_page: usize,
        }

        let accounts = GoogleAccount::get_for_user(user, conn).await?;
        let mut channels = Vec::default();

        let client = reqwest::Client::default();

        for mut account in accounts {
            let req = client
                .get("https://www.googleapis.com/youtube/v3/channels?part=snippet,statistics&mine=true&maxResults=50")
                .headers(account.bearer_header(conn).await?)
                .build()?;
            let resp: Response = client.execute(req).await?.json().await?;
            assert!(resp.page_info.total_results <= resp.page_info.results_per_page);

            channels.extend(resp.items);
        }

        Ok(channels)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelSnippet {
    pub title: String,
    pub custom_url: String,
    pub thumbnails: HashMap<String, Thumbnail>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnail {
    pub url: String,
    pub width: usize,
    pub height: usize,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelStatistics {
    #[serde(deserialize_with = "deserialize_usize_from_string")]
    pub view_count: usize,
    #[serde(deserialize_with = "deserialize_usize_from_string")]
    pub subscriber_count: usize,
    #[serde(deserialize_with = "deserialize_usize_from_string")]
    pub video_count: usize,
}

fn deserialize_usize_from_string<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum StringOrUsize {
        String(String),
        Number(usize),
    }

    match StringOrUsize::deserialize(deserializer)? {
        StringOrUsize::String(s) => s.parse().map_err(serde::de::Error::custom),
        StringOrUsize::Number(i) => Ok(i),
    }
}
