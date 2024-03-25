use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::COOKIE, request::Parts},
};
use diesel::{pg::Pg, prelude::*};
use diesel_async::{
    pooled_connection::deadpool::Pool, AsyncConnection, AsyncPgConnection, RunQueryDsl,
};
use oauth2::RefreshToken;
use rand::Rng;
use time::{Duration, OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

use crate::twitch::TwitchSession;
use crate::{google::GoogleSession, utils::oauth::OAuthAccountHelper, Error, SESSION_COOKIE_NAME};
const BUFFER_TIME: Duration = Duration::seconds(1);

#[derive(Clone, Copy, Insertable, Queryable)]
#[diesel(table_name = crate::schema::inneruser)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: Uuid,
}

impl User {
    pub async fn new(conn: &mut impl AsyncConnection<Backend = Pg>) -> Result<Self, Error> {
        let user = User { id: Uuid::new_v4() };

        use crate::schema::inneruser::dsl as dsl_iu;

        diesel::insert_into(dsl_iu::inneruser)
            .values(user)
            .execute(conn)
            .await?;

        Ok(user)
    }
}

#[async_trait]
impl FromRequestParts<Pool<AsyncPgConnection>> for User {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        pool: &Pool<AsyncPgConnection>,
    ) -> Result<Self, Self::Rejection> {
        if let Some(cookies) = parts.headers.get(COOKIE) {
            let parts = cookies.as_bytes().split(|c| *c == b';');
            for part in parts {
                if let Ok(part) = std::str::from_utf8(part) {
                    let part = part.trim();

                    if let Some((name, value)) = part.split_once('=') {
                        if name == SESSION_COOKIE_NAME {
                            let mut conn = pool.get().await?;

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

#[derive(Insertable, Queryable)]
#[diesel(table_name = crate::schema::inneruserdata)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserData {
    pub given_name: String,
    pub family_name: String,
    pub banner_desc: String,
}

impl UserData {
    pub async fn from_user(
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Option<Self>, Error> {
        use crate::schema::inneruserdata::dsl as dsl_iud;

        let user = dsl_iud::inneruserdata
            .select((
                dsl_iud::given_name,
                dsl_iud::family_name,
                dsl_iud::banner_desc,
            ))
            .filter(dsl_iud::id.eq(user.id))
            .first(conn)
            .await
            .optional()?;

        Ok(user)
    }
}

#[derive(Clone, Insertable, Queryable)]
#[diesel(table_name = crate::schema::innerusersession)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserSession {
    pub token: String,
    pub expires_at: PrimitiveDateTime,
    pub user_id: Uuid,
}

impl UserSession {
    pub async fn new_for_user(
        user: User,
        expires_at: PrimitiveDateTime,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Self, Error> {
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

        diesel::insert_into(crate::schema::innerusersession::dsl::innerusersession)
            .values(session.clone())
            .execute(conn)
            .await?;

        Ok(session)
    }

    pub async fn get_user_by_token(
        token: &str,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Option<User>, Error> {
        use crate::schema::innerusersession::dsl as dsl_ius;

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

        use crate::schema::innerusersession::dsl as dsl_ius;

        diesel::delete(dsl_ius::innerusersession)
            .filter(dsl_ius::expires_at.lt(now))
            .execute(conn)
            .await?;

        Ok(())
    }
}

#[derive(Insertable, Queryable)]
#[diesel(table_name = crate::schema::twitchaccount)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TwitchAccount {
    pub id: String,
    pub access_token: String,
    pub expires_at: PrimitiveDateTime,
    pub refresh_token: String,
    pub user_id: Uuid,
}

impl TwitchAccount {
    pub fn meta(&self) -> TwitchAccountMeta {
        TwitchAccountMeta {
            id: self.id.clone(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchAccountMeta {
    pub id: String,
}

#[derive(Clone, Insertable, Queryable, AsChangeset)]
#[diesel(table_name = crate::schema::googleaccount)]
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
        use crate::schema::googleaccount::dsl as dsl_ga;

        let user = dsl_ga::googleaccount
            .filter(dsl_ga::sub.eq(sub))
            .first(conn)
            .await
            .optional()?;

        Ok(user)
    }

    pub async fn list(
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Self>, Error> {
        use crate::schema::googleaccount::dsl as dsl_ga;

        let accounts = dsl_ga::googleaccount
            .filter(dsl_ga::user_id.eq(user.id))
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
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAccountMeta {
    pub sub: String,
    pub email: String,
}

pub trait AuthenticationHeader {
    type Session: OAuthAccountHelper;

    fn access_token(&self) -> &str;
    fn expires_at(&self) -> PrimitiveDateTime;
    fn refresh_token(&self) -> String;
    fn user(&self) -> User;
    fn update(&mut self, session: Self::Session);

    async fn authentication_header(
        &mut self,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<reqwest::header::HeaderMap, Error> {
        let now = OffsetDateTime::now_utc();
        if (PrimitiveDateTime::new(now.date(), now.time()) + BUFFER_TIME) > self.expires_at() {
            let session = Self::Session::renew(RefreshToken::new(self.refresh_token())).await?;

            session.insert_or_update_for_user(self.user(), conn).await?;

            self.update(session);
        }

        let mut map = reqwest::header::HeaderMap::new();
        map.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.access_token()))
                .expect("Failed to make the bearer token header value"),
        );

        Ok(map)
    }
}

impl AuthenticationHeader for GoogleAccount {
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

impl AuthenticationHeader for TwitchAccount {
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
