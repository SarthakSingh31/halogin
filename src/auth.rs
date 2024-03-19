use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::COOKIE, request::Parts},
    response::IntoResponse,
};
use diesel::{
    insert_into,
    query_dsl::methods::{FilterDsl, SelectDsl},
    ExpressionMethods,
};
use diesel_async::{pooled_connection::deadpool::PoolError, RunQueryDsl};
use uuid::Uuid;

pub struct User(Uuid);

impl User {
    pub async fn create_new() -> Result<Self, DbError> {
        let id = Uuid::new_v4();
        let mut conn = crate::POOL.get().await?;

        use crate::schema::inneruser::dsl as dsl_iu;

        insert_into(dsl_iu::inneruser)
            .values(dsl_iu::id.eq(id))
            .execute(&mut conn)
            .await?;

        Ok(User(id))
    }
}

pub enum Authentication {
    Unauthenticated,
    Authenticated { user: User },
}

impl Authentication {
    const AUTH_HEADER: &'static [u8] = b"HALOGIN-SESSION";
}

#[async_trait]
impl<S> FromRequestParts<S> for Authentication
where
    S: Send + Sync,
{
    type Rejection = DbError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(cookies) = parts.headers.get(COOKIE) {
            let mut parts = cookies.as_bytes().split(|c| *c == b';');
            while let Some(part) = parts.next() {
                let mut kv = part.split(|c| *c == b'=');

                if let Some(key) = kv.next() {
                    if key == Self::AUTH_HEADER {
                        if let Some(value) = kv.next() {
                            if let Ok(token) = std::str::from_utf8(value) {
                                let mut conn = crate::POOL.get().await?;

                                use crate::schema::innerusersession::dsl as dsl_ius;

                                let user = dsl_ius::innerusersession
                                    .select(dsl_ius::user_id)
                                    .filter(dsl_ius::token.eq(token))
                                    .first(&mut conn)
                                    .await?;

                                return Ok(Authentication::Authenticated { user: User(user) });
                            }
                        }
                    }
                }
            }

            Ok(Authentication::Unauthenticated)
        } else {
            Ok(Authentication::Unauthenticated)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Failed to get connection from pool: {0:?}")]
    PoolError(#[from] PoolError),
    #[error("Failed to user using the token from the DB: {0:?}")]
    QueryError(#[from] diesel::result::Error),
}

impl IntoResponse for DbError {
    fn into_response(self) -> axum::response::Response {
        (
            axum::http::status::StatusCode::INTERNAL_SERVER_ERROR,
            axum::response::Html(format!("{self:?}")),
        )
            .into_response()
    }
}
