use axum::{routing, Json, Router};
use diesel::pg::Pg;
use diesel_async::AsyncConnection;
use oauth2::{AccessToken, RefreshToken};
use time::PrimitiveDateTime;

use crate::{
    db::{TwitchAccount, User},
    state::DbConn,
    utils::{oauth::OAuthAccountHelper, AuthenticationHeader},
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
    ) -> Result<(), Error> {
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
        .route("/account", routing::get(get_twitch_accounts))
}

#[derive(serde::Serialize)]
struct Account {
    id: usize,
    display_name: String,
    profile_image_url: String,
    follower_count: usize,
    subscriber_count: usize,
}

async fn get_twitch_accounts(
    user: User,
    DbConn { mut conn }: DbConn,
) -> Result<Json<Vec<Account>>, Error> {
    let mut accounts = Vec::default();

    let client = reqwest::Client::new();
    for mut account in TwitchAccount::list(user, &mut conn).await? {
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
        let user_req = client
            .get(format!(
                "https://api.twitch.tv/helix/users?id={}",
                account.id
            ))
            .headers(account.headers(&mut conn).await?)
            .build()?;
        let mut user_resp: UserResp = client.execute(user_req).await?.json().await?;

        #[derive(serde::Deserialize)]
        struct SubsriberResp {
            total: usize,
        }
        let subscriber_req = client
            .get(format!(
                "https://api.twitch.tv/helix/subscriptions?broadcaster_id={}",
                account.id
            ))
            .headers(account.headers(&mut conn).await?)
            .build()?;
        let subscriber_resp: SubsriberResp = client.execute(subscriber_req).await?.json().await?;

        #[derive(serde::Deserialize)]
        struct FollowerResp {
            total: usize,
        }
        let follower_req = client
            .get(format!(
                "https://api.twitch.tv/helix/channels/followers?broadcaster_id={}",
                account.id
            ))
            .headers(account.headers(&mut conn).await?)
            .build()?;
        let follower_resp: FollowerResp = client.execute(follower_req).await?.json().await?;

        let user_resp = user_resp
            .data
            .pop()
            .expect("Got no users in the user response");

        accounts.push(Account {
            id: user_resp.id,
            display_name: user_resp.display_name,
            profile_image_url: user_resp.profile_image_url,
            follower_count: follower_resp.total,
            subscriber_count: subscriber_resp.total,
        });
    }

    Ok(Json(accounts))
}
