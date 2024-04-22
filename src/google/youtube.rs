use std::collections::HashMap;

use axum::{http::HeaderMap, Json};
use futures::StreamExt;

use crate::{
    db::{GoogleAccount, GoogleAccountMeta, User},
    state::DbConn,
    utils::{AuthenticationHeader, GetDetail},
    Error,
};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: String,
    pub snippet: ChannelSnippet,
    pub statistics: ChannelStatistics,
    pub account: GoogleAccountMeta,
}

impl GetDetail for Vec<Channel> {
    type Account = GoogleAccount;

    async fn get<'g>(
        account: &'g mut Self::Account,
        client: &'g reqwest::Client,
        headers: HeaderMap,
    ) -> Result<Self, Error> {
        #[derive(serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct ResponseChannel {
            pub id: String,
            pub snippet: ChannelSnippet,
            pub statistics: ChannelStatistics,
        }

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            page_info: PageInfo,
            items: Option<Vec<ResponseChannel>>,
        }

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PageInfo {
            total_results: usize,
            results_per_page: usize,
        }

        let req = client
                .get("https://www.googleapis.com/youtube/v3/channels?part=snippet,statistics&mine=true&maxResults=50")
                .headers(headers)
                .build()?;
        let resp: Response = client.execute(req).await?.json().await?;
        assert!(resp.page_info.total_results <= resp.page_info.results_per_page);

        let meta = account.meta();
        Ok(resp
            .items
            .unwrap_or_default()
            .into_iter()
            .map(move |channel| Channel {
                id: channel.id,
                snippet: channel.snippet,
                statistics: channel.statistics,
                account: meta.clone(),
            })
            .collect())
    }
}

impl Channel {
    pub async fn list(user: User, DbConn { mut conn }: DbConn) -> Result<Json<Vec<Self>>, Error> {
        let accounts = GoogleAccount::list(user, &mut conn).await?;
        let mut channels = Vec::default();

        let mut acc_and_headers = Vec::with_capacity(accounts.len());
        for mut account in accounts {
            let headers = account.headers(&mut conn).await?;
            acc_and_headers.push((account, headers));
        }
        let mut channels_iter = futures::stream::iter(acc_and_headers.into_iter())
            .map(|(mut account, headers)| {
                let client = reqwest::Client::default();
                async move { Vec::<Self>::get(&mut account, &client, headers).await }
            })
            .buffer_unordered(10);

        while let Some(channel) = channels_iter.next().await {
            channels.extend(channel?);
        }

        Ok(Json(channels))
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
    #[serde(deserialize_with = "crate::utils::deserialize_usize_from_string")]
    pub view_count: usize,
    #[serde(deserialize_with = "crate::utils::deserialize_usize_from_string")]
    pub subscriber_count: usize,
    #[serde(deserialize_with = "crate::utils::deserialize_usize_from_string")]
    pub video_count: usize,
}
