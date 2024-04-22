use axum::{http::HeaderMap, routing, Json, Router};
use diesel::pg::Pg;
use diesel_async::AsyncConnection;
use futures::StreamExt;
use oauth2::{AccessToken, RefreshToken};
use time::PrimitiveDateTime;

use crate::{
    db::{TwitchAccount, User},
    state::DbConn,
    utils::{oauth::OAuthAccountHelper, AuthenticationHeader, GetDetail},
    Error,
};

#[derive(Debug, Clone)]
pub struct TwitchSession {
    access_token: AccessToken,
    expires_at: PrimitiveDateTime,
    refresh_token: RefreshToken,
    id: String,
}

impl TwitchSession {
    pub fn access_token(&self) -> String {
        self.access_token.secret().clone()
    }

    pub fn expires_at(&self) -> PrimitiveDateTime {
        self.expires_at
    }

    pub fn refresh_token(&self) -> String {
        self.refresh_token.secret().clone()
    }
}

impl OAuthAccountHelper for TwitchSession {
    const CLIENT_ID: &'static str = "<TwitchID>";
    const CLIENT_SECRET: &'static str = "<TwitchSecret>";
    const AUTH_URL: &'static str = "https://id.twitch.tv/oauth2/authorize";
    const TOKEN_URL: &'static str = "https://id.twitch.tv/oauth2/token";
    const AUTH_TYPE: oauth2::AuthType = oauth2::AuthType::RequestBody;

    type ExtraFields = oauth2::EmptyExtraTokenFields;
    type Account = TwitchAccount;
    type Response = Account;

    async fn new(
        access_token: AccessToken,
        expires_at: PrimitiveDateTime,
        refresh_token: RefreshToken,
        _extra_fields: &Self::ExtraFields,
    ) -> Result<Self, Error> {
        let client = reqwest::Client::new();

        #[derive(serde::Deserialize)]
        struct Resp {
            data: Vec<Data>,
        }
        #[derive(serde::Deserialize)]
        struct Data {
            id: String,
        }
        let req = client
            .get("https://api.twitch.tv/helix/users")
            .bearer_auth(access_token.secret())
            .header("Client-Id", Self::CLIENT_ID)
            .build()?;
        let mut resp: Resp = client.execute(req).await?.json().await?;

        Ok(TwitchSession {
            access_token,
            expires_at,
            refresh_token,
            id: resp
                .data
                .pop()
                .expect("Twitch user response does not have any user data")
                .id,
        })
    }

    async fn insert_or_update_for_user(
        &self,
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Self::Account, Error> {
        TwitchAccount {
            id: self.id.clone(),
            access_token: self.access_token.secret().clone(),
            expires_at: self.expires_at,
            refresh_token: self.refresh_token.secret().clone(),
            user_id: user.id,
        }
        .insert_or_update(conn)
        .await
    }
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new()
        .route("/login", routing::post(TwitchSession::login))
        .route("/account", routing::get(Account::list))
}

#[derive(serde::Serialize)]
pub struct Account {
    id: usize,
    display_name: String,
    profile_image_url: String,
    follower_count: usize,
    subscriber_count: usize,
}

impl GetDetail for Account {
    type Account = TwitchAccount;

    async fn get<'g>(
        account: &'g mut Self::Account,
        client: &'g reqwest::Client,
        headers: HeaderMap,
    ) -> Result<Self, Error> {
        #[derive(serde::Deserialize)]
        struct UserResp {
            data: Vec<UserData>,
        }
        #[derive(serde::Deserialize)]
        struct UserData {
            #[serde(deserialize_with = "crate::utils::deserialize_usize_from_string")]
            id: usize,
            display_name: String,
            profile_image_url: String,
        }
        #[derive(serde::Deserialize)]
        struct SubsriberResp {
            total: usize,
        }
        #[derive(serde::Deserialize)]
        struct FollowerResp {
            total: usize,
        }

        let user_req = client
            .get(format!(
                "https://api.twitch.tv/helix/users?id={}",
                account.id
            ))
            .headers(headers.clone())
            .build()?;
        let subscriber_req = client
            .get(format!(
                "https://api.twitch.tv/helix/subscriptions?broadcaster_id={}",
                account.id
            ))
            .headers(headers.clone())
            .build()?;
        let follower_req = client
            .get(format!(
                "https://api.twitch.tv/helix/channels/followers?broadcaster_id={}",
                account.id
            ))
            .headers(headers)
            .build()?;

        let (user_resp, subscriber_resp, follower_resp): (
            Result<UserResp, Error>,
            Result<SubsriberResp, Error>,
            Result<FollowerResp, Error>,
        ) = tokio::join!(
            async { Ok(client.execute(user_req).await?.json().await?) },
            async { Ok(client.execute(subscriber_req).await?.json().await?) },
            async { Ok(client.execute(follower_req).await?.json().await?) },
        );

        let user_resp = user_resp?
            .data
            .pop()
            .expect("Got no users in the user response");

        Ok(Account {
            id: user_resp.id,
            display_name: user_resp.display_name,
            profile_image_url: user_resp.profile_image_url,
            follower_count: follower_resp?.total,
            subscriber_count: subscriber_resp?.total,
        })
    }
}

impl Account {
    async fn list(user: User, DbConn { mut conn }: DbConn) -> Result<Json<Vec<Account>>, Error> {
        let accounts = TwitchAccount::list(user, &mut conn).await?;

        let mut acc_and_headers = Vec::with_capacity(accounts.len());
        for mut account in accounts {
            let headers = account.headers(&mut conn).await?;
            acc_and_headers.push((account, headers));
        }
        let mut accounts = Vec::default();
        let mut accounts_iter = futures::stream::iter(acc_and_headers.into_iter())
            .map(|(mut account, headers)| {
                let client = reqwest::Client::default();
                async move { Self::get(&mut account, &client, headers).await }
            })
            .buffer_unordered(10);

        while let Some(account) = accounts_iter.next().await {
            accounts.push(account?);
        }

        Ok(Json(accounts))
    }
}
