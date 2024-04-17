mod youtube;

use axum::{routing, Router};
use diesel::pg::Pg;
use diesel_async::AsyncConnection;
use oauth2::{AccessToken, ExtraTokenFields, RefreshToken};
use time::PrimitiveDateTime;

use crate::{
    db::{GoogleAccount, User},
    utils::oauth::OAuthAccountHelper,
    Error,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct IdToken {
    id_token: String,
}

impl ExtraTokenFields for IdToken {}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct IdTokenDecoded {
    sub: String,
    email: String,
}

#[derive(Debug, Clone)]
pub struct GoogleSession {
    access_token: AccessToken,
    expires_at: PrimitiveDateTime,
    refresh_token: RefreshToken,
    email: String,
    sub: String,
}

impl GoogleSession {
    pub fn access_token(&self) -> String {
        self.access_token.secret().clone()
    }

    pub fn expires_at(&self) -> PrimitiveDateTime {
        self.expires_at
    }

    pub fn refresh_token(&self) -> String {
        self.refresh_token.secret().clone()
    }

    pub fn email(&self) -> String {
        self.email.clone()
    }
}

impl OAuthAccountHelper for GoogleSession {
    const CLIENT_ID: &'static str =
        "751704262503-61e56pavvl5d8l5fg6s62iejm8ft16ac.apps.googleusercontent.com";
    const CLIENT_SECRET: &'static str = "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";
    const AUTH_URL: &'static str = "https://accounts.google.com/o/oauth2/v2/auth";
    const TOKEN_URL: &'static str = "https://oauth2.googleapis.com/token";
    const AUTH_TYPE: oauth2::AuthType = oauth2::AuthType::BasicAuth;

    type ExtraFields = IdToken;

    async fn new(
        access_token: AccessToken,
        expires_at: PrimitiveDateTime,
        refresh_token: RefreshToken,
        extra_fields: &Self::ExtraFields,
    ) -> Result<Self, Error> {
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.insecure_disable_signature_validation();
        validation.validate_aud = false;
        validation.validate_exp = false;

        let id_token_decoded = jsonwebtoken::decode::<IdTokenDecoded>(
            &extra_fields.id_token,
            &jsonwebtoken::DecodingKey::from_secret(&[]),
            &validation,
        )
        .expect("With verification disabled this is infallible");

        Ok(GoogleSession {
            access_token,
            expires_at,
            refresh_token,
            email: id_token_decoded.claims.email,
            sub: id_token_decoded.claims.sub,
        })
    }

    async fn insert_or_update_for_user(
        &self,
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<(), Error> {
        GoogleAccount {
            sub: self.sub.clone(),
            email: self.email.clone(),
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
        .route("/login", routing::post(GoogleSession::login))
        .route("/youtube/channel", routing::get(youtube::Channel::list))
}
