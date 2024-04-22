#![feature(duration_constructors)]
#![feature(lazy_cell)]
#![feature(let_chains)]

mod chat;
mod company;
mod creator;
mod db;
mod google;
pub mod models;
pub mod schema;
mod search;
mod state;
mod storage;
mod twitch;
mod utils;
mod ws;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing, Router,
};
use diesel::pg::Pg;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use time::Duration;
use tokio::sync::mpsc;
use tower_http::services::ServeDir;

pub const SESSION_COOKIE_NAME: &str = "HALOGIN-SESSION";
pub const SESSION_COOKIE_DURATION: Duration = Duration::days(90);

pub const MAINTENANCE_INTERVAL: std::time::Duration = std::time::Duration::from_days(1);

pub async fn run() {
    tracing_subscriber::fmt::init();

    let db_url = &*dotenvy::var("DATABASE_URL")
        .expect("Failed to get DATABASE_URL")
        .leak();
    let storage_path = std::path::Path::new(
        &*dotenvy::var("STORAGE_PATH")
            .expect("Failed to get STORAGE_PATH")
            .leak(),
    );

    tokio::spawn(async move {
        async fn maintain(conn: &mut impl AsyncConnection<Backend = Pg>) -> Result<(), Error> {
            db::UserSession::prune_expired(conn).await?;

            diesel::sql_query("REINDEX INDEX CONCURRENTLY creator_profile_embedding;")
                .execute(conn)
                .await?;
            diesel::sql_query("VACUUM CreatorProfile;")
                .execute(conn)
                .await?;

            diesel::sql_query("REINDEX INDEX CONCURRENTLY company_embedding;")
                .execute(conn)
                .await?;
            diesel::sql_query("VACUUM Company;").execute(conn).await?;

            Ok(())
        }

        loop {
            match AsyncPgConnection::establish(db_url).await {
                Ok(mut conn) => {
                    if let Err(err) = maintain(&mut conn).await {
                        tracing::warn!("{err:?}");
                    }
                }
                Err(err) => {
                    tracing::warn!("{err:?}");
                }
            }

            tokio::time::sleep(MAINTENANCE_INTERVAL).await;
        }
    });

    let mut fcm_client = fcm::Client::new()
        .await
        .expect("Failed to build fcm::Client");
    let (fcm_tx, mut fcm_rx) = mpsc::unbounded_channel();
    let state = state::AppState::new(
        db_url,
        fcm_tx.clone(),
        ws::WsFunctions::default().add_scoped("chat", chat::functions()),
        state::Config { storage_path },
    )
    .await;

    let pool = state.pool.clone();
    tokio::spawn(async move {
        while let Some(msg) = fcm_rx.recv().await {
            if let Err(err) = fcm_client.send(&msg).await {
                match err {
                    fcm::Error::InvalidMessage(err) => match &msg.target {
                        fcm::Target::Token(token) => match pool.get().await {
                            Ok(mut conn) => {
                                if let Err(err) =
                                    models::SessionFcmToken::delete(token, &mut conn).await
                                {
                                    tracing::error!("Failed to delete old fcm token: {err:?}")
                                }
                            }
                            Err(err) => {
                                tracing::error!("Failed to get connection from pool: {err:?}")
                            }
                        },
                        target => {
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
        .nest("/api/v1/creator", creator::router())
        .nest("/api/v1/company", company::router())
        .nest("/api/v1/google", google::router())
        .nest("/api/v1/twitch", twitch::router())
        .nest("/api/v1/storage", storage::router())
        .nest_service(
            "/",
            utils::AddHtmlExtService(ServeDir::new("frontend/build")),
        )
        .route("/test/:id", axum::routing::get(test))
        .route("/ws", routing::get(ws::connect))
        .with_state(state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Started server on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
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
    #[error("Error while trying to decode a file upload: {0:?}")]
    UploadError(#[from] axum::extract::multipart::MultipartError),
    #[error("Encountered a filesystem error: {0:?}")]
    FsError(#[from] std::io::Error),
    #[error("Encountered an error in tokio: {0:?}")]
    TokioError(#[from] tokio::task::JoinError),
    #[error("Encountered an error while saving the image: {0:?}")]
    SaveImageError(#[from] image::ImageError),
    #[error("Encountered an error in cradle: {0:?}")]
    CandleError(#[from] candle_core::Error),
    #[error("Encountered an error in hugging face api: {0:?}")]
    HuggingFaceError(#[from] hf_hub::api::tokio::ApiError),
    #[error("Encountered an error in tokenizer: {0:?}")]
    TokenizerError(tokenizers::Error),
    #[error("Encountered an error in Qdrant: {0:?}")]
    QdrantError(anyhow::Error),
    #[error("Failed to convert header while trying to fetch a image: {0:?}")]
    HeaderCoversionError(axum::http::header::ToStrError),
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
            _ => (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("{self:?}"))).into_response(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Platform {
    Youtube {
        subscribers: usize,
        average_viewers: usize,
        top_countries: Vec<String>,
    },
    Twitch {
        followers: usize,
        subscribers: usize,
        average_viewers: usize,
    },
}

async fn test(Path(id): Path<String>, State(state): State<state::AppState>) {
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

        // state.fcm_tx.send(message).expect("Failed to send message");
        todo!()
    });
}
