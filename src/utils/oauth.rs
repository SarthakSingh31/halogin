use axum::{
    http::{header::SET_COOKIE, HeaderName, StatusCode},
    Json,
};
use axum_extra::{either::Either, extract::cookie::Cookie};
use diesel::pg::Pg;
use diesel_async::AsyncConnection;
use oauth2::{
    basic::{
        BasicErrorResponse, BasicRevocationErrorResponse, BasicTokenIntrospectionResponse,
        BasicTokenType,
    },
    AccessToken, AuthType, AuthUrl, Client, ClientId, ClientSecret, ExtraTokenFields, RedirectUrl,
    RefreshToken, StandardRevocableToken, TokenResponse, TokenType, TokenUrl,
};
use time::{OffsetDateTime, PrimitiveDateTime};

use crate::{
    db::{User, UserSession},
    state::DbConn,
    Error,
};

use super::{AuthenticationHeader, GetDetail};

#[derive(serde::Deserialize)]
pub struct LoginParams {
    redirect_origin: String,
    code: String,
    keep_logged_in: bool,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct MinimalTokenResponse<EF, TT>
where
    EF: ExtraTokenFields,
    TT: TokenType,
{
    access_token: AccessToken,
    #[serde(bound = "TT: TokenType")]
    #[serde(deserialize_with = "oauth2::helpers::deserialize_untagged_enum_case_insensitive")]
    token_type: TT,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<RefreshToken>,

    #[serde(bound = "EF: ExtraTokenFields")]
    #[serde(flatten)]
    extra_fields: EF,
}

impl<EF, TT> TokenResponse for MinimalTokenResponse<EF, TT>
where
    EF: ExtraTokenFields,
    TT: TokenType,
{
    type TokenType = TT;

    fn access_token(&self) -> &AccessToken {
        &self.access_token
    }

    fn token_type(&self) -> &Self::TokenType {
        &self.token_type
    }

    fn expires_in(&self) -> Option<std::time::Duration> {
        self.expires_in.map(std::time::Duration::from_secs)
    }

    fn refresh_token(&self) -> Option<&RefreshToken> {
        self.refresh_token.as_ref()
    }

    fn scopes(&self) -> Option<&Vec<oauth2::Scope>> {
        None
    }
}

pub trait OAuthAccountHelper: Sized {
    const CLIENT_ID: &'static str;
    const CLIENT_SECRET: &'static str;
    const AUTH_URL: &'static str;
    const TOKEN_URL: &'static str;
    const AUTH_TYPE: AuthType;

    type ExtraFields: ExtraTokenFields;
    type Account: AuthenticationHeader;
    type Response: serde::Serialize + GetDetail<Account = Self::Account>;

    fn new(
        access_token: AccessToken,
        expires_at: PrimitiveDateTime,
        refresh_token: RefreshToken,
        extra_fields: &Self::ExtraFields,
    ) -> impl futures::Future<Output = Result<Self, Error>> + Send + Sync;

    async fn insert_or_update_for_user(
        &self,
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Self::Account, Error>;

    async fn from_code(redirect_url: String, code: String) -> Result<Self, Error> {
        let client = Client::<
            BasicErrorResponse,
            MinimalTokenResponse<Self::ExtraFields, BasicTokenType>,
            BasicTokenIntrospectionResponse,
            StandardRevocableToken,
            BasicRevocationErrorResponse,
        >::new(ClientId::new(Self::CLIENT_ID.into()))
        .set_auth_type(Self::AUTH_TYPE)
        .set_client_secret(ClientSecret::new(Self::CLIENT_SECRET.into()))
        .set_auth_uri(AuthUrl::new(Self::AUTH_URL.into())?)
        .set_token_uri(TokenUrl::new(Self::TOKEN_URL.into())?)
        .set_redirect_uri(RedirectUrl::new(redirect_url).map_err(|err| Error::Custom {
            status_code: StatusCode::BAD_REQUEST,
            error: format!("Failed to parse redirect url: {err:?}"),
        })?);

        let auth = client
            .exchange_code(oauth2::AuthorizationCode::new(code))
            .request_async(&reqwest::Client::default())
            .await
            .map_err(|err| Error::Custom {
                status_code: StatusCode::BAD_REQUEST,
                error: format!("Could not get the tokens from the provided code: {err:?}"),
            })?;

        assert_eq!(*auth.token_type(), BasicTokenType::Bearer);

        let expires_at = auth
            .expires_in()
            .map(|duration| OffsetDateTime::now_utc() + duration)
            .ok_or(Error::Custom {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                error: "Failed to get an expiry time for the given code".to_string(),
            })?;
        let refresh_token = auth.refresh_token().cloned().ok_or(Error::Custom {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            error: "Could not get a refresh token for the given code".to_string(),
        })?;

        Self::new(
            auth.access_token().clone(),
            PrimitiveDateTime::new(expires_at.date(), expires_at.time()),
            refresh_token,
            &auth.extra_fields,
        )
        .await
    }

    async fn renew(refresh_token: RefreshToken) -> Result<Self, Error> {
        let client = Client::<
            BasicErrorResponse,
            MinimalTokenResponse<Self::ExtraFields, BasicTokenType>,
            BasicTokenIntrospectionResponse,
            StandardRevocableToken,
            BasicRevocationErrorResponse,
        >::new(ClientId::new(Self::CLIENT_ID.into()))
        .set_auth_type(Self::AUTH_TYPE)
        .set_client_secret(ClientSecret::new(Self::CLIENT_SECRET.into()))
        .set_auth_uri(AuthUrl::new(Self::AUTH_URL.into())?)
        .set_token_uri(TokenUrl::new(Self::TOKEN_URL.into())?);

        let resp = client
            .exchange_refresh_token(&refresh_token)
            .request_async(&reqwest::Client::default())
            .await
            .map_err(|err| Error::Custom {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                error: format!("Failed to exchange refresh token for a new access token: {err:?}"),
            })?;

        assert_eq!(*resp.token_type(), BasicTokenType::Bearer);

        let expires_at = resp
            .expires_in()
            .map(|duration| OffsetDateTime::now_utc() + duration)
            .ok_or(Error::Custom {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                error: "Failed to get an expiry time for the given code".to_string(),
            })?;
        let refresh_token = resp.refresh_token().cloned().unwrap_or(refresh_token);

        Self::new(
            resp.access_token().clone(),
            PrimitiveDateTime::new(expires_at.date(), expires_at.time()),
            refresh_token,
            &resp.extra_fields,
        )
        .await
    }

    async fn login(
        user: Option<User>,
        DbConn { mut conn }: DbConn,
        Json(login_params): Json<LoginParams>,
    ) -> Result<
        Either<Json<Self::Response>, ([(HeaderName, String); 2], Json<Self::Response>)>,
        Error,
    > {
        let session = Self::from_code(login_params.redirect_origin, login_params.code).await?;

        let resp = if let Some(user) = user {
            let mut acct = session.insert_or_update_for_user(user, &mut conn).await?;

            let headers = acct.headers(&mut conn).await?;
            Either::E1(Json(
                Self::Response::get(&mut acct, &reqwest::Client::new(), headers).await?,
            ))
        } else {
            let now = OffsetDateTime::now_utc();
            let expires_at =
                PrimitiveDateTime::new(now.date(), now.time()) + crate::SESSION_COOKIE_DURATION;

            let user = User::new(&mut conn).await?;
            let mut acct = session.insert_or_update_for_user(user, &mut conn).await?;

            let session = UserSession::new_for_user(user, expires_at, &mut conn).await?;

            let mut session_cookie = Cookie::new(crate::SESSION_COOKIE_NAME, session.token);
            let mut user_id_cookie = Cookie::new(crate::USER_ID_COOKIE_NAME, user.id.to_string());

            session_cookie.set_secure(true);
            session_cookie.set_http_only(true);
            session_cookie.set_path("/");
            user_id_cookie.set_path("/");
            if login_params.keep_logged_in {
                let expire_time = OffsetDateTime::new_utc(expires_at.date(), expires_at.time());
                session_cookie.set_expires(expire_time);
                user_id_cookie.set_expires(expire_time);
            }

            let headers = acct.headers(&mut conn).await?;
            Either::E2((
                [
                    (SET_COOKIE, session_cookie.encoded().to_string()),
                    (SET_COOKIE, user_id_cookie.encoded().to_string()),
                ],
                Json(Self::Response::get(&mut acct, &reqwest::Client::new(), headers).await?),
            ))
        };

        Ok(resp)
    }
}
