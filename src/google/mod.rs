mod youtube;

use axum::{routing, Json, Router};
use diesel::pg::Pg;
use diesel_async::AsyncConnection;
use futures::StreamExt;
use oauth2::{AccessToken, ExtraTokenFields, RefreshToken};
use time::PrimitiveDateTime;

use crate::{
    db::{GoogleAccount, User},
    state::DbConn,
    utils::{oauth::OAuthAccountHelper, AuthenticationHeader},
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
    const CLIENT_ID: &'static str = "<GoogleID>";
    const CLIENT_SECRET: &'static str = "<GoogleSecret>";
    const AUTH_URL: &'static str = "https://accounts.google.com/o/oauth2/v2/auth";
    const TOKEN_URL: &'static str = "https://oauth2.googleapis.com/token";
    const AUTH_TYPE: oauth2::AuthType = oauth2::AuthType::BasicAuth;

    type ExtraFields = IdToken;
    type Account = GoogleAccount;
    type Response = Vec<youtube::Channel>;

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
    ) -> Result<Self::Account, Error> {
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
        .route("/profile_photo", routing::get(ProfilePhoto::list))
        .route("/youtube/channel", routing::get(youtube::Channel::list))
}

#[derive(serde::Serialize)]
struct ProfilePhoto {
    primary: bool,
    url: String,
}

impl ProfilePhoto {
    async fn list(
        user: User,
        DbConn { mut conn }: DbConn,
    ) -> Result<Json<Vec<ProfilePhoto>>, Error> {
        #[derive(serde::Deserialize)]
        struct Response {
            photos: Vec<Photo>,
        }

        #[derive(serde::Deserialize)]
        struct Photo {
            metadata: PhotoMetadata,
            url: String,
        }

        #[derive(serde::Deserialize)]
        struct PhotoMetadata {
            #[serde(default)]
            primary: bool,
        }

        let mut photos = Vec::default();
        let client = reqwest::Client::default();

        let accounts = GoogleAccount::list(user, &mut conn).await?;

        let mut account_headers = Vec::with_capacity(accounts.len());
        for mut account in accounts {
            let headers = account.headers(&mut conn).await?;
            account_headers.push(headers);
        }

        let mut responses = futures::stream::iter(account_headers)
            .map(|headers| {
                let client = client.clone();
                async move {
                    let req = client
                        .get("https://people.googleapis.com/v1/people/me?personFields=photos")
                        .headers(headers)
                        .build()?;
                    let resp: Response = client.execute(req).await?.json().await?;

                    Result::<_, Error>::Ok(resp)
                }
            })
            .buffer_unordered(10);

        while let Some(response) = responses.next().await {
            for photo in response?.photos {
                photos.push(ProfilePhoto {
                    primary: photo.metadata.primary,
                    url: photo.url,
                });
            }
        }

        return Ok(Json(photos));
    }
}
