mod youtube;

use axum::{routing, Router};
use diesel::{pg::Pg, ExpressionMethods};
use diesel_async::{
    pooled_connection::deadpool::Pool, AsyncConnection, AsyncPgConnection, RunQueryDsl,
};
use oauth2::{AccessToken, ExtraTokenFields, RefreshToken};
use time::PrimitiveDateTime;

use crate::{
    models::{GoogleAccount, User},
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

    type ExtraFields = IdToken;

    fn new(
        access_token: AccessToken,
        expires_at: PrimitiveDateTime,
        refresh_token: RefreshToken,
        extra_fields: &Self::ExtraFields,
    ) -> Self {
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

        GoogleSession {
            access_token,
            expires_at,
            refresh_token,
            email: id_token_decoded.claims.email,
            sub: id_token_decoded.claims.sub,
        }
    }

    async fn insert_or_update_for_user(
        &self,
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<(), Error> {
        use crate::schema::googleaccount::dsl as dsl_ga;

        diesel::insert_into(dsl_ga::googleaccount)
            .values(GoogleAccount {
                sub: self.sub.clone(),
                email: self.email.clone(),
                access_token: self.access_token.secret().clone(),
                expires_at: self.expires_at,
                refresh_token: self.refresh_token.secret().clone(),
                user_id: user.id,
            })
            .on_conflict(dsl_ga::sub)
            .do_update()
            .set((
                dsl_ga::email.eq(self.email.clone()),
                dsl_ga::access_token.eq(self.access_token.secret()),
                dsl_ga::expires_at.eq(&self.expires_at),
                dsl_ga::refresh_token.eq(self.refresh_token.secret()),
                dsl_ga::user_id.eq(&user.id),
            ))
            .execute(conn)
            .await?;

        Ok(())
    }
}

pub fn router() -> Router<Pool<AsyncPgConnection>> {
    Router::new()
        .route("/login", routing::post(GoogleSession::login))
        .route("/channel/list", routing::get(youtube::Channel::list))
}
