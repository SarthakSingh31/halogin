use axum::{routing, Router};
use diesel::{pg::Pg, ExpressionMethods};
use diesel_async::{
    pooled_connection::deadpool::Pool, AsyncConnection, AsyncPgConnection, RunQueryDsl,
};
use oauth2::{AccessToken, ExtraTokenFields, RefreshToken};
use time::PrimitiveDateTime;

use crate::{
    models::{TwitchAccount, User},
    utils::oauth::OAuthAccountHelper,
    Error,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TwitchIdToken {
    id_token: String,
}

impl ExtraTokenFields for TwitchIdToken {}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TwitchIdTokenDecoded {
    id: String,
}

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
    const CLIENT_ID: &'static str = "65x8qdhtinpz5889thff2ae4o0nxrw";
    const CLIENT_SECRET: &'static str = "shxqoc1j7dlzd0yj6z9ro9en5iaqdk";
    const AUTH_URL: &'static str = "https://id.twitch.tv/oauth2/authorize";
    const TOKEN_URL: &'static str = "https://id.twitch.tv/oauth2/token";

    type ExtraFields = TwitchIdToken;

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

        let id_token_decoded = jsonwebtoken::decode::<TwitchIdTokenDecoded>(
            &extra_fields.id_token,
            &jsonwebtoken::DecodingKey::from_secret(&[]),
            &validation,
        )
        .expect("With verification disabled this is infallible");

        TwitchSession {
            access_token,
            expires_at,
            refresh_token,
            id: id_token_decoded.claims.id,
        }
    }

    async fn insert_or_update_for_user(
        &self,
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<(), Error> {
        use crate::schema::twitchaccount::dsl as dsl_ta;

        diesel::insert_into(dsl_ta::twitchaccount)
            .values(TwitchAccount {
                id: self.id.clone(),
                access_token: self.access_token.secret().clone(),
                expires_at: self.expires_at.clone(),
                refresh_token: self.refresh_token.secret().clone(),
                user_id: user.id,
            })
            .on_conflict(dsl_ta::id)
            .do_update()
            .set((
                dsl_ta::access_token.eq(self.access_token.secret()),
                dsl_ta::expires_at.eq(&self.expires_at),
                dsl_ta::refresh_token.eq(self.refresh_token.secret()),
                dsl_ta::user_id.eq(user.id),
            ))
            .execute(conn)
            .await?;

        Ok(())
    }
}
pub fn router() -> Router<Pool<AsyncPgConnection>> {
    Router::new().route("/login", routing::post(TwitchSession::login))
}
