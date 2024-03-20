use std::sync::LazyLock;

use axum::http::StatusCode;
use diesel::{pg::Pg, ExpressionMethods};
use diesel_async::{AsyncConnection, RunQueryDsl};
use oauth2::{
    basic::{
        BasicErrorResponse, BasicRevocationErrorResponse, BasicTokenIntrospectionResponse,
        BasicTokenType,
    },
    revocation::StandardRevocableToken,
    AccessToken, AuthUrl, Client, ClientId, ClientSecret, ExtraTokenFields, RedirectUrl,
    RefreshToken, StandardTokenResponse, TokenResponse, TokenUrl,
};
use time::{OffsetDateTime, PrimitiveDateTime};

use crate::{models::User, Error};

static AUTH_URL: LazyLock<AuthUrl> = LazyLock::new(|| {
    AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".into())
        .expect("Failed to parse account url")
});
static TOKEN_URL: LazyLock<TokenUrl> = LazyLock::new(|| {
    TokenUrl::new("https://oauth2.googleapis.com/token".into()).expect("Failed to parse token url")
});

type GoogleClient = Client<
    BasicErrorResponse,
    StandardTokenResponse<IdToken, BasicTokenType>,
    BasicTokenType,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
>;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct IdToken {
    id_token: String,
}

impl ExtraTokenFields for IdToken {}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct IdTokenDecoded {
    sub: String,
}

#[derive(Debug, Clone)]
pub struct GoogleSession {
    bearer_access_token: AccessToken,
    expires_at: PrimitiveDateTime,
    refresh_token: RefreshToken,
    sub: String,
}

impl GoogleSession {
    const CLIENT_ID: &'static str =
        "751704262503-61e56pavvl5d8l5fg6s62iejm8ft16ac.apps.googleusercontent.com";
    const CLIENT_SECRET: &'static str = "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";

    pub async fn from_code(redirect_url: String, code: String) -> Result<Self, Error> {
        println!("{}, {:?}", redirect_url, redirect_url);
        let client = GoogleClient::new(
            ClientId::new(Self::CLIENT_ID.into()),
            Some(ClientSecret::new(Self::CLIENT_SECRET.into())),
            AUTH_URL.clone(),
            Some(TOKEN_URL.clone()),
        )
        .set_redirect_uri(RedirectUrl::new(redirect_url).map_err(|err| Error::Custom {
            status_code: StatusCode::BAD_REQUEST,
            error: format!("Failed to parse redirect url: {err:?}"),
        })?);

        let auth = client
            .exchange_code(oauth2::AuthorizationCode::new(code))
            .request_async(oauth2::reqwest::async_http_client)
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
                error: format!("Failed to get an expiry time for the given code"),
            })?;
        let refresh_token =
            auth.refresh_token()
                .map(|token| token.clone())
                .ok_or(Error::Custom {
                    status_code: StatusCode::INTERNAL_SERVER_ERROR,
                    error: format!("Could not get a refresh token for the given code"),
                })?;

        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.insecure_disable_signature_validation();
        validation.validate_aud = false;
        validation.validate_exp = false;

        let id_token_decoded = jsonwebtoken::decode::<IdTokenDecoded>(
            &auth.extra_fields().id_token,
            &jsonwebtoken::DecodingKey::from_secret(&[]),
            &validation,
        )
        .expect("With verification disabled this is infallible");

        Ok(GoogleSession {
            bearer_access_token: auth.access_token().clone(),
            expires_at: PrimitiveDateTime::new(expires_at.date(), expires_at.time()),
            refresh_token,
            sub: id_token_decoded.claims.sub,
        })
    }

    pub fn sub(&self) -> &str {
        &self.sub
    }

    pub async fn insert_or_update_for_user(
        &self,
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<(), Error> {
        use crate::schema::googleuser::dsl as dsl_gu;

        diesel::insert_into(dsl_gu::googleuser)
            .values(crate::models::GoogleUser {
                sub: self.sub.clone(),
                access_token: self.bearer_access_token.secret().clone(),
                expires_at: self.expires_at.clone(),
                refresh_token: self.refresh_token.secret().clone(),
                user_id: user.id,
            })
            .on_conflict(dsl_gu::sub)
            .do_update()
            .set((
                dsl_gu::access_token.eq(self.bearer_access_token.secret()),
                dsl_gu::expires_at.eq(&self.expires_at),
                dsl_gu::refresh_token.eq(self.refresh_token.secret()),
                dsl_gu::user_id.eq(&user.id),
            ))
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn bearer_header(&mut self) -> reqwest::header::HeaderValue {
        todo!()
    }
}
