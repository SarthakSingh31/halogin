use std::convert::Infallible;
use std::task::{Context, Poll};

use crate::{db::User, Error};
use axum::body::Bytes;
use axum::http::{HeaderValue, Request, Response};
use diesel::pg::Pg;
use diesel_async::AsyncConnection;
use time::{Duration, OffsetDateTime, PrimitiveDateTime};
use tower::Service;
use tower_http::services::ServeDir;

pub mod oauth;

use oauth::OAuthAccountHelper;

const BUFFER_TIME: Duration = Duration::seconds(1);

pub trait AuthenticationHeader {
    const EXTRA_HEADERS: Self::ExtraHeader;

    type ExtraHeader: IntoIterator<Item = (&'static str, HeaderValue)>;
    type Session: OAuthAccountHelper;

    fn access_token(&self) -> &str;
    fn expires_at(&self) -> PrimitiveDateTime;
    fn refresh_token(&self) -> String;
    fn user(&self) -> User;
    fn update(&mut self, session: Self::Session);

    fn headers(
        &mut self,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> impl futures::Future<Output = Result<reqwest::header::HeaderMap, Error>> {
        async move {
            let now = OffsetDateTime::now_utc();
            if (PrimitiveDateTime::new(now.date(), now.time()) + BUFFER_TIME) > self.expires_at() {
                let session =
                    Self::Session::renew(oauth2::RefreshToken::new(self.refresh_token())).await?;

                session.insert_or_update_for_user(self.user(), conn).await?;

                self.update(session);
            }

            let mut map = reqwest::header::HeaderMap::new();
            map.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.access_token()))
                    .expect("Failed to make the bearer token header value"),
            );
            for (key, val) in Self::EXTRA_HEADERS {
                map.insert(key, val.clone());
            }

            Ok(map)
        }
    }
}

/// Service that automatically adding .html extension to requests
#[derive(Debug, Clone)]
pub struct AddHtmlExtService<Fallback>(pub ServeDir<Fallback>);

impl<ReqBody, F, FResBody> Service<Request<ReqBody>> for AddHtmlExtService<F>
where
    F: Service<Request<ReqBody>, Response = Response<FResBody>, Error = Infallible> + Clone,
    F::Future: Send + 'static,
    FResBody: http_body::Body<Data = Bytes> + Send + 'static,
    FResBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = <ServeDir<F> as Service<Request<ReqBody>>>::Response;
    type Error = <ServeDir<F> as Service<Request<ReqBody>>>::Error;
    type Future = <ServeDir<F> as Service<Request<ReqBody>>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <ServeDir<F> as Service<Request<ReqBody>>>::poll_ready(&mut self.0, cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        if let Some(path) = req.uri().path_and_query() {
            let path = path.path();
            if let Some(end_part) = path.split('/').last() {
                if !end_part.is_empty() && !end_part.contains('.') {
                    // this removes the scheme and authority, but it's ok since ServeDir doesn't care
                    if let Ok(uri) = format!("{path}.html").parse() {
                        *req.uri_mut() = uri;
                    }
                }
            }
        }

        self.0.call(req)
    }
}

pub fn deserialize_usize_from_string<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum StringOrUsize {
        String(String),
        Number(usize),
    }

    use serde::Deserialize;

    match StringOrUsize::deserialize(deserializer)? {
        StringOrUsize::String(s) => s.parse().map_err(serde::de::Error::custom),
        StringOrUsize::Number(i) => Ok(i),
    }
}
