#![feature(duration_constructors)]
#![feature(lazy_cell)]

mod chat;
mod google;
mod models;
mod rpc;
mod schema;
mod twitch;
mod utils;

use std::sync::Arc;

use axum::{
    async_trait,
    extract::{FromRequestParts, Path, State},
    http::{request::Parts, StatusCode},
    response::{Html, IntoResponse},
    Router,
};
use dashmap::DashMap;
use diesel::pg::Pg;
use diesel_async::{
    pooled_connection::{
        deadpool::{Object, Pool},
        AsyncDieselConnectionManager,
    },
    AsyncConnection, AsyncPgConnection,
};
use models::User;
use slotmap::{DefaultKey, DenseSlotMap};
use time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

const SESSION_COOKIE_NAME: &str = "HALOGIN-SESSION";
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

    let mut fcm_client = fcm::Client::new()
        .await
        .expect("Failed to build fcm::Client");
    let (fcm_tx, mut fcm_rx) = mpsc::unbounded_channel();
    let state = AppState::new(db_url, fcm_tx.clone()).await;

    let pool = state.pool.clone();
    tokio::spawn(async move {
        while let Some(msg) = fcm_rx.recv().await {
            if let Err(err) = fcm_client.send(&msg).await {
                match err {
                    fcm::Error::InvalidMessage(err) => match &msg.target {
                        fcm::Target::Token(token) => match pool.get().await {
                            Ok(mut conn) => {
                                if let Err(err) =
                                    models::UserFcmToken::delete(token, &mut conn).await
                                {
                                    tracing::error!("Failed to delete old fcm token: {err:?}")
                                }
                            }
                            Err(err) => {
                                tracing::error!("Failed to get connection from pool: {err:?}")
                            }
                        },
                        target @ _ => {
                            tracing::error!("Failed to send message with target: {target:?} with error: {err:?}");
                        }
                    },
                    fcm::Error::ServerError(Some(retry_after)) => {
                        let fcm_tx = fcm_tx.clone();
                        tokio::spawn(async move {
                            let delay = match retry_after {
                                fcm::RetryAfter::Delay(delay) => delay,
                                fcm::RetryAfter::DateTime(date_time) => {
                                    date_time - time::OffsetDateTime::now_utc()
                                }
                            };

                            // Making the delay non negative and then waiting for that duration
                            tokio::time::sleep(
                                delay
                                    .clamp(time::Duration::ZERO, time::Duration::MAX)
                                    .unsigned_abs(),
                            )
                            .await;

                            if fcm_tx.send(msg).is_err() {
                                tracing::error!(
                                    "Failed to re-queue a message after it was set to retry"
                                );
                            }
                        });
                    }
                    _ => tracing::error!("Failed to send message over fcm: {err:?}"),
                }
            }
        }
    });

    let app = Router::new()
        .nest("/api/v1/google", google::router())
        .nest("/api/v1/twitch", twitch::router())
        .nest(
            "/rpc",
            rpc::router(rpc::RpcServer::default().add_module("chat", chat::module)),
        )
        .nest_service("/", tower_http::services::ServeDir::new("frontend/build"))
        .route("/test/:id", axum::routing::get(test))
        .with_state(state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Clone, Copy)]
pub struct AppState {
    pool: &'static Pool<AsyncPgConnection>,
    rpc: &'static DashMap<Uuid, DenseSlotMap<DefaultKey, mpsc::UnboundedSender<serde_json::Value>>>,
    fcm_tx: &'static mpsc::UnboundedSender<fcm::Message>,
}

impl AppState {
    async fn new(db_url: &str, fcm_tx: mpsc::UnboundedSender<fcm::Message>) -> Self {
        Self {
            pool: {
                let config = AsyncDieselConnectionManager::new(db_url);

                let pool = Pool::<AsyncPgConnection>::builder(config)
                    .build()
                    .expect("Failed to build the pool");

                Box::leak(Box::new(pool))
            },
            rpc: Box::leak(Box::new(Default::default())),
            fcm_tx: Box::leak(Box::new(fcm_tx)),
        }
    }

    pub async fn get_conn(&self) -> Result<impl AsyncConnection<Backend = Pg>, Error> {
        self.pool.get().await.map_err(|err| err.into())
    }

    pub fn insert_user_tx(
        &self,
        user: User,
        tx: mpsc::UnboundedSender<serde_json::Value>,
    ) -> DefaultKey {
        let mut txs = self.rpc.entry(user.id).or_default();
        txs.insert(tx)
    }

    pub fn remove(&self, user: User, key: DefaultKey) {
        let mut txs = self.rpc.entry(user.id).or_default();
        txs.remove(key);
    }

    pub fn send(&self, user: User, value: serde_json::Value) {
        if let Some(streams) = self.rpc.get(&user.id) {
            for (_, stream) in streams.iter() {
                if stream.send(value.clone()).is_err() {
                    tracing::error!(
                        "Failed to send message to user.\nUser: {}\nMessage: {value}",
                        user.id
                    );
                }
            }
        }
    }
}

pub struct PgConn {
    pub conn: Object<AsyncPgConnection>,
}

#[async_trait]
impl FromRequestParts<AppState> for PgConn {
    type Rejection = Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let conn = state.pool.get().await?;
        Ok(PgConn { conn })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Request must be made from an authenticated session")]
    Unauthorized,
    #[error("The requested RPC namespace does not exist")]
    RpcMissingNamespace,
    #[error("The requested RPC method does not exist in the given namespace")]
    RpcMissingMethod,
    #[error("An error occured: {status_code:?} => {error}")]
    Custom {
        status_code: StatusCode,
        error: String,
    },
    #[error("Failed to get connection from pool: {0:?}")]
    PoolError(#[from] diesel_async::pooled_connection::deadpool::PoolError),
    #[error("Failed to query using the token from the DB: {0:?}")]
    QueryError(#[from] diesel::result::Error),
    #[error("Failed to make a request: {0:?}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Failed to parse url: {0:?}")]
    ParseError(#[from] url::ParseError),
    #[error("Failed to parse json: {0:?}")]
    SerdeJsonError(#[from] serde_json::Error),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::Unauthorized => {
                (StatusCode::UNAUTHORIZED, Html(format!("{self:?}"))).into_response()
            }
            Error::RpcMissingNamespace | Error::RpcMissingMethod | Error::SerdeJsonError(_) => {
                (StatusCode::BAD_REQUEST, Html(format!("{self:?}"))).into_response()
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

async fn test(Path(id): Path<String>, State(state): State<AppState>) {
    println!("Called test");

    let data = serde_json::json!({
        "key": "value",
    });

    let message = fcm::Message {
        data: Some(data.clone()),
        notification: Some(fcm::Notification {
            title: Some("I'm high".to_string()),
            body: Some(format!("it's {}", time::OffsetDateTime::now_utc())),
            ..Default::default()
        }),
        target: fcm::Target::Token(id),
        fcm_options: None,
        android: None,
        apns: None,
        webpush: None,
    };

    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        state.fcm_tx.send(message).expect("Failed to send message");
    });
}
