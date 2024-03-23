#![feature(duration_constructors)]
#![feature(lazy_cell)]

mod google;
mod models;
mod oauth;
mod schema;
mod twitch;

use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
    Router,
};
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncConnection, AsyncPgConnection,
};
use time::Duration;

const SESSION_COOKIE_NAME: &'static str = "HALOGIN-SESSION";
const SESSION_COOKIE_DURATION: Duration = Duration::days(90);

const PRUNE_INTERVAL: std::time::Duration = std::time::Duration::from_days(1);

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let db_url = &*dotenvy::var("DATABASE_URL")
        .expect("Failed to get DATABASE_URL")
        .leak();

    tokio::spawn(async move {
        loop {
            match AsyncPgConnection::establish(db_url).await {
                Ok(mut conn) => {
                    if let Err(err) = models::UserSession::prune_expired(&mut conn).await {
                        tracing::warn!("{err:?}");
                    }
                }
                Err(err) => {
                    tracing::warn!("{err:?}");
                }
            }

            tokio::time::sleep(PRUNE_INTERVAL).await;
        }
    });

    let app = Router::new()
        .nest("/api/v1/google", google::router())
        .nest_service("/", tower_http::services::ServeDir::new("frontend/build"))
        .nest("/api/v1/twitch", twitch::router()) 
        .with_state({
            let config = AsyncDieselConnectionManager::new(db_url);

            Pool::<AsyncPgConnection>::builder(config)
                .build()
                .expect("Failed to build the pool")
        });

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Request must be made from an authenticated session")]
    Unauthorized,
    #[error("An error occured: {status_code:?} => {error}")]
    Custom {
        status_code: StatusCode,
        error: String,
    },
    #[error("Failed to get connection from pool: {0:?}")]
    PoolError(#[from] diesel_async::pooled_connection::deadpool::PoolError),
    #[error("Failed to user using the token from the DB: {0:?}")]
    QueryError(#[from] diesel::result::Error),
    #[error("Failed to make a request: {0:?}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Failed to parse url: {0:?}")]
    ParseError(#[from] url::ParseError),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::Unauthorized => {
                (StatusCode::UNAUTHORIZED, Html(format!("{self:?}"))).into_response()
            }
            Error::Custom { status_code, error } => (status_code, Html(error)).into_response(),
            Error::PoolError(_)
            | Error::QueryError(_)
            | Error::ReqwestError(_)
            | Error::ParseError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("{self:?}"))).into_response()
            }
        }
    }
}
