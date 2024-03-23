use axum::http::StatusCode;
use diesel::{pg::Pg, ExpressionMethods};
use diesel_async::{AsyncConnection, RunQueryDsl};
use oauth2::{
    basic::{
        BasicErrorResponse, BasicRevocationErrorResponse, BasicTokenIntrospectionResponse,
        BasicTokenType,
    }, revocation::StandardRevocableToken, AccessToken, AuthUrl, Client, ClientId, ClientSecret, ExtraTokenFields, RedirectUrl, StandardTokenResponse, TokenResponse, TokenUrl
};
use serde::{Serialize,Deserialize};
use time::{OffsetDateTime, PrimitiveDateTime};

use crate::{models::User, Error};

use std::sync::LazyLock;

static AUTH_URL: LazyLock<AuthUrl> = LazyLock::new(|| {
    AuthUrl::new("https://id.twitch.tv/oauth2/authorize".into())
        .expect("Failed to parse auth url")
});
static TOKEN_URL: LazyLock<TokenUrl> = LazyLock::new(|| {
    TokenUrl::new("https://id.twitch.tv/oauth2/token".into())
        .expect("Failed to parse token url")
});

type TwitchClient = Client<
    BasicErrorResponse,
    StandardTokenResponse<IdToken,BasicTokenType>,
    BasicTokenType,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
>;

#[derive(Debug, Serialize, Deserialize)]
pub struct IdToken {
    id_token: String,
}
impl ExtraTokenFields for IdToken {}

#[derive(Debug)]
pub struct TwitchSession {
    bearer_access_token: AccessToken,
    expires_at: PrimitiveDateTime,
}


impl TwitchSession {
    const CLIENT_ID: &'static str = "65x8qdhtinpz5889thff2ae4o0nxrw";
    const CLIENT_SECRET: &'static str = "shxqoc1j7dlzd0yj6z9ro9en5iaqdk";

    pub async fn from_code(redirect_url: String, code: String) -> Result<Self, Error> {
        let client = TwitchClient::new(
            ClientId::new(Self::CLIENT_ID.into()),
            Some(ClientSecret::new(Self::CLIENT_SECRET.into())),
            AUTH_URL.clone(),
            Some(TOKEN_URL.clone()),
        )
        .set_redirect_uri(RedirectUrl::new(redirect_url).map_err(|err| Error::Custom {
            status_code: StatusCode::BAD_REQUEST,
            error: format!("Failed to parse redirect url: {:?}", err),
        })?);

        let auth = client
            .exchange_code(oauth2::AuthorizationCode::new(code))
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|err| Error::Custom {
                status_code: StatusCode::BAD_REQUEST,
                error: format!("Could not get the tokens from the provided code: {:?}", err),
            })?;

        assert_eq!(*auth.token_type(), BasicTokenType::Bearer);

        let expires_at = auth
            .expires_in()
            .map(|duration| OffsetDateTime::now_utc() + duration)
            .ok_or(Error::Custom {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                error: format!("Failed to get an expiry time for the given code"),
            })?;

        Ok(TwitchSession {
            bearer_access_token: auth.access_token().clone(),
            expires_at: PrimitiveDateTime::new(expires_at.date(), expires_at.time()),
        })
    }

    pub async fn insert_or_update_for_user(
        &self,
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<(), Error> {
        todo!()

        // Ok(())
    }

    pub async fn bearer_header(&mut self) -> reqwest::header::HeaderValue {
       
        todo!()
    }
}
