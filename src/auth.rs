use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::COOKIE, request::Parts},
};

use crate::{
    models::{User, UserSession},
    Error, SESSION_COOKIE_NAME,
};

pub enum Authentication {
    Unauthenticated,
    Authenticated { user: User },
}

#[async_trait]
impl<S> FromRequestParts<S> for Authentication
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(cookies) = parts.headers.get(COOKIE) {
            let mut parts = cookies.as_bytes().split(|c| *c == b';');
            while let Some(part) = parts.next() {
                let mut kv = part.split(|c| *c == b'=');

                if let Some(key) = kv.next() {
                    if key == SESSION_COOKIE_NAME.as_bytes() {
                        if let Some(value) = kv.next() {
                            if let Ok(token) = std::str::from_utf8(value) {
                                let mut conn = crate::POOL.get().await?;

                                // We ignore the session cookie if we cannot find a session associated with it
                                if let Some(user) =
                                    UserSession::get_user_by_token(token, &mut conn).await?
                                {
                                    return Ok(Authentication::Authenticated { user });
                                }
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
