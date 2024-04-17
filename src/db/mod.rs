use std::borrow::Cow;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::CONTENT_TYPE, request::Parts, HeaderValue, StatusCode},
};
use diesel::{
    deserialize::Queryable, pg::Pg, prelude::Insertable, upsert::excluded, AsChangeset,
    ExpressionMethods, OptionalExtension, QueryDsl, Selectable,
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use image::{DynamicImage, ImageFormat};
use pgvector::Vector;
use time::{OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

use crate::{
    google::GoogleSession,
    state::AppState,
    storage::Storage,
    twitch::TwitchSession,
    utils::{oauth::OAuthAccountHelper, AuthenticationHeader},
    Error,
};

mod embedding;
mod schema;
mod sql_types;

#[derive(Clone, Copy)]
pub struct Encoder(&'static embedding::EmbeddingEncoder);

impl Encoder {
    pub async fn new() -> Self {
        Encoder(Box::leak(Box::new(
            embedding::EmbeddingEncoder::new()
                .await
                .expect("Failed to build the embedding encoder"),
        )))
    }

    pub async fn encode(&self, text: String) -> Result<Vec<f32>, Error> {
        self.0.encode(text).await
    }
}

#[derive(Clone, Copy, Insertable, Queryable, Selectable)]
#[diesel(table_name = schema::inneruser)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: Uuid,
}

impl User {
    pub async fn new(conn: &mut impl AsyncConnection<Backend = Pg>) -> Result<Self, Error> {
        let user = User { id: Uuid::new_v4() };

        use schema::inneruser::dsl as dsl_iu;

        diesel::insert_into(dsl_iu::inneruser)
            .values(user)
            .execute(conn)
            .await?;

        Ok(user)
    }
}

#[async_trait]
impl FromRequestParts<AppState> for User {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(cookies) = parts.headers.get(axum::http::header::COOKIE) {
            let parts = cookies.as_bytes().split(|c| *c == b';');
            for part in parts {
                if let Ok(part) = std::str::from_utf8(part) {
                    let part = part.trim();

                    if let Some((name, value)) = part.split_once('=') {
                        if name == crate::SESSION_COOKIE_NAME {
                            let mut conn = state.get_conn().await?;

                            // We ignore the session cookie if we cannot find a session associated with it
                            if let Some(user) =
                                UserSession::get_user_by_token(value, &mut conn).await?
                            {
                                return Ok(user);
                            }
                        }
                    }
                }
            }
        }

        Err(Error::Unauthorized)
    }
}

#[derive(Clone, Insertable, Queryable)]
#[diesel(table_name = schema::innerusersession)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserSession {
    pub token: Cow<'static, str>,
    pub expires_at: PrimitiveDateTime,
    pub user_id: Uuid,
}

impl UserSession {
    pub async fn new_for_user(
        user: User,
        expires_at: PrimitiveDateTime,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Self, Error> {
        use rand::Rng;

        let token = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(256)
            .map(char::from)
            .collect();
        let session = UserSession {
            token,
            expires_at,
            user_id: user.id,
        };

        diesel::insert_into(schema::innerusersession::dsl::innerusersession)
            .values(session.clone())
            .execute(conn)
            .await?;

        Ok(session)
    }

    pub async fn get_user_by_token(
        token: &str,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Option<User>, Error> {
        use schema::innerusersession::dsl as dsl_ius;

        let now = OffsetDateTime::now_utc();
        let now = PrimitiveDateTime::new(now.date(), now.time());

        let user = dsl_ius::innerusersession
            .select((dsl_ius::user_id,))
            .filter(dsl_ius::token.eq(token))
            .filter(dsl_ius::expires_at.gt(now))
            .first(conn)
            .await
            .optional()?;

        Ok(user)
    }

    pub async fn prune_expired(conn: &mut impl AsyncConnection<Backend = Pg>) -> Result<(), Error> {
        let now = OffsetDateTime::now_utc();
        let now = PrimitiveDateTime::new(now.date(), now.time());

        use schema::innerusersession::dsl as dsl_ius;

        diesel::delete(dsl_ius::innerusersession)
            .filter(dsl_ius::expires_at.lt(now))
            .execute(conn)
            .await?;

        Ok(())
    }
}

#[derive(Clone, Insertable)]
#[diesel(table_name = schema::creatordata)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CreatorData<'d> {
    user_id: Uuid,
    pub given_name: &'d str,
    pub family_name: &'d str,
    pub pronouns: &'d str,
    pub profile_desc: &'d str,
    pub content_desc: &'d str,
    pub audience_desc: &'d str,
    pfp_path: Option<&'d str>,
    embedding: Vector,
}

impl<'d> CreatorData<'d> {
    fn format_creator_descriptions(profile: &str, content: &str, audience: &str) -> String {
        format!("# Content Creator Profile Description:\n{profile}\n\n# Content Creator Content Description:\n{content}\n\n# Content Creator Audience Description:\n{audience}")
    }

    pub async fn insert_update(
        user: User,
        given_name: &str,
        family_name: &str,
        pronouns: &str,
        profile_desc: &str,
        content_desc: &str,
        audience_desc: &str,
        pfp_hidden: Option<&str>,
        pfp: Option<(DynamicImage, ImageFormat)>,
        conn: &mut impl AsyncConnection<Backend = Pg>,
        encoder: Encoder,
        storage: Storage,
    ) -> Result<(), Error> {
        let user_embedding_desc =
            Self::format_creator_descriptions(profile_desc, content_desc, audience_desc);
        let embedding = encoder.encode(user_embedding_desc).await?;

        let pfp_path = if let Some(url) = pfp_hidden
            && !url.is_empty()
        {
            let response = reqwest::get(url).await?;
            let mime_type = response
                .headers()
                .get(CONTENT_TYPE)
                .ok_or(Error::Custom {
                    status_code: StatusCode::BAD_REQUEST,
                    error: format!("Could not figure out image content type from the url request."),
                })?
                .to_str()
                .map_err(Error::HeaderCoversionError)?
                .to_string();
            let img_bytes = response.bytes().await?;
            let format = ImageFormat::from_mime_type(&mime_type).ok_or(Error::Custom {
                status_code: StatusCode::BAD_REQUEST,
                error: format!("Could not figure out image format from mime type: {mime_type}"),
            })?;

            let pfp = image::load_from_memory_with_format(&img_bytes, format)?;

            Some(storage.store_public_pfp(user, pfp, format).await?)
        } else {
            if let Some((pfp, format)) = pfp {
                Some(storage.store_public_pfp(user, pfp, format).await?)
            } else {
                None
            }
        };

        use schema::creatordata::dsl as cd_dsl;

        diesel::insert_into(cd_dsl::creatordata)
            .values(&CreatorData {
                user_id: user.id,
                given_name,
                family_name,
                pronouns,
                profile_desc,
                content_desc,
                audience_desc,
                pfp_path: pfp_path.as_ref().map(|path| path.as_str()),
                embedding: embedding.into(),
            })
            .on_conflict(cd_dsl::user_id)
            .do_update()
            .set((
                cd_dsl::given_name.eq(excluded(cd_dsl::given_name)),
                cd_dsl::family_name.eq(excluded(cd_dsl::family_name)),
                cd_dsl::pronouns.eq(excluded(cd_dsl::pronouns)),
                cd_dsl::profile_desc.eq(excluded(cd_dsl::profile_desc)),
                cd_dsl::content_desc.eq(excluded(cd_dsl::content_desc)),
                cd_dsl::audience_desc.eq(excluded(cd_dsl::audience_desc)),
                cd_dsl::pfp_path.eq(excluded(cd_dsl::pfp_path)),
                cd_dsl::embedding.eq(excluded(cd_dsl::embedding)),
            ))
            .execute(conn)
            .await?;

        Ok(())
    }
}

#[derive(Insertable, Queryable)]
#[diesel(table_name = schema::twitchaccount)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TwitchAccount {
    pub id: String,
    pub access_token: String,
    pub expires_at: PrimitiveDateTime,
    pub refresh_token: String,
    pub user_id: Uuid,
}

impl TwitchAccount {
    pub async fn list(
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Self>, Error> {
        use schema::twitchaccount::dsl as ta_dsl;

        let accounts = ta_dsl::twitchaccount
            .filter(ta_dsl::user_id.eq(user.id))
            .load(conn)
            .await?;

        Ok(accounts)
    }

    pub fn meta(&self) -> TwitchAccountMeta {
        TwitchAccountMeta {
            id: self.id.clone(),
        }
    }

    pub async fn insert_or_update(
        self,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Self, Error> {
        use schema::twitchaccount::dsl as ta_dsl;

        diesel::insert_into(ta_dsl::twitchaccount)
            .values(&self)
            .on_conflict(ta_dsl::id)
            .do_update()
            .set((
                ta_dsl::access_token.eq(excluded(ta_dsl::access_token)),
                ta_dsl::expires_at.eq(excluded(ta_dsl::expires_at)),
                ta_dsl::refresh_token.eq(excluded(ta_dsl::refresh_token)),
                ta_dsl::user_id.eq(excluded(ta_dsl::user_id)),
            ))
            .execute(conn)
            .await?;

        Ok(self)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchAccountMeta {
    pub id: String,
}

impl AuthenticationHeader for TwitchAccount {
    const EXTRA_HEADERS: Self::ExtraHeader = [(
        "Client-Id",
        HeaderValue::from_static(TwitchSession::CLIENT_ID),
    )];

    type ExtraHeader = [(&'static str, HeaderValue); 1];
    type Session = TwitchSession;

    fn access_token(&self) -> &str {
        &self.access_token
    }

    fn expires_at(&self) -> PrimitiveDateTime {
        self.expires_at
    }

    fn refresh_token(&self) -> String {
        self.refresh_token.clone()
    }

    fn user(&self) -> User {
        User { id: self.user_id }
    }

    fn update(&mut self, session: Self::Session) {
        self.access_token = session.access_token();
        self.expires_at = session.expires_at();
        self.refresh_token = session.refresh_token();
        // session.id does not change so we don't need to update it
    }
}

#[derive(Clone, Insertable, Queryable, AsChangeset)]
#[diesel(table_name = schema::googleaccount)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GoogleAccount {
    pub sub: String,
    pub email: String,
    pub access_token: String,
    pub expires_at: PrimitiveDateTime,
    pub refresh_token: String,
    pub user_id: Uuid,
}

impl GoogleAccount {
    pub async fn from_sub(
        sub: &str,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Option<Self>, Error> {
        use schema::googleaccount::dsl as ga_dsl;

        let user = ga_dsl::googleaccount
            .filter(ga_dsl::sub.eq(sub))
            .first(conn)
            .await
            .optional()?;

        Ok(user)
    }

    pub async fn list(
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Self>, Error> {
        use schema::googleaccount::dsl as ga_dsl;

        let accounts = ga_dsl::googleaccount
            .filter(ga_dsl::user_id.eq(user.id))
            .load(conn)
            .await?;

        Ok(accounts)
    }

    pub fn meta(&self) -> GoogleAccountMeta {
        GoogleAccountMeta {
            sub: self.sub.clone(),
            email: self.email.clone(),
        }
    }

    pub async fn insert_or_update(
        self,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Self, Error> {
        use schema::googleaccount::dsl as ga_dsl;

        diesel::insert_into(ga_dsl::googleaccount)
            .values(&self)
            .on_conflict(ga_dsl::sub)
            .do_update()
            .set((
                ga_dsl::email.eq(excluded(ga_dsl::email)),
                ga_dsl::access_token.eq(excluded(ga_dsl::access_token)),
                ga_dsl::expires_at.eq(excluded(ga_dsl::expires_at)),
                ga_dsl::refresh_token.eq(excluded(ga_dsl::refresh_token)),
                ga_dsl::user_id.eq(excluded(ga_dsl::user_id)),
            ))
            .execute(conn)
            .await?;

        Ok(self)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAccountMeta {
    pub sub: String,
    pub email: String,
}

impl AuthenticationHeader for GoogleAccount {
    const EXTRA_HEADERS: Self::ExtraHeader = [];

    type ExtraHeader = [(&'static str, HeaderValue); 0];
    type Session = GoogleSession;

    fn access_token(&self) -> &str {
        &self.access_token
    }

    fn expires_at(&self) -> PrimitiveDateTime {
        self.expires_at
    }

    fn refresh_token(&self) -> String {
        self.refresh_token.clone()
    }

    fn user(&self) -> User {
        User { id: self.user_id }
    }

    fn update(&mut self, session: Self::Session) {
        self.access_token = session.access_token();
        self.expires_at = session.expires_at();
        self.refresh_token = session.refresh_token();
        self.email = session.email();
        // session.sub does not change so we don't need to update it
    }
}
