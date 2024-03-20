#![feature(duration_constructors)]
#![feature(lazy_cell)]

mod auth;
mod google;
mod models;
mod schema;

use std::sync::LazyLock;

use axum::{
    extract::Path,
    http::{header::SET_COOKIE, HeaderName, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing, Json, Router,
};
use cookie::Cookie;
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};
use models::{GoogleUser, User, UserData};
use serde::Deserialize;
use time::{Duration, OffsetDateTime, PrimitiveDateTime};

const SESSION_COOKIE_NAME: &'static str = "HALOGIN-SESSION";
const SESSION_COOKIE_DURATION: Duration = Duration::days(90);

const PRUNE_INTERVAL: std::time::Duration = std::time::Duration::from_days(1);

static POOL: LazyLock<Pool<AsyncPgConnection>> = LazyLock::new(|| {
    let config = AsyncDieselConnectionManager::new(
        dotenvy::var("DATABASE_URL").expect("Failed to get DATABASE_URL"),
    );
    Pool::builder(config)
        .build()
        .expect("Failed to build the pool")
});

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    tokio::spawn(async move {
        loop {
            match POOL.get().await {
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
        .route("/login/:endpoint", routing::post(login))
        .route("/attach/:endpoint", routing::post(attach))
        .nest_service("/", tower_http::services::ServeDir::new("frontend/build"));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum AuthEndpoint {
    Google,
    Twitch,
}

#[derive(Deserialize)]
struct LoginParams {
    redirect_origin: String,
    code: String,
}

async fn login(
    Path(endpoint): Path<AuthEndpoint>,
    Json(login_params): Json<LoginParams>,
) -> Result<([(HeaderName, String); 1], Redirect), Error> {
    let now = OffsetDateTime::now_utc();
    let expires_at = PrimitiveDateTime::new(now.date(), now.time()) + SESSION_COOKIE_DURATION;

    match endpoint {
        AuthEndpoint::Google => {
            let session =
                google::GoogleSession::from_code(login_params.redirect_origin, login_params.code)
                    .await?;

            let mut conn = POOL.get().await?;
            let user =
                if let Some(google_user) = GoogleUser::from_sub(session.sub(), &mut conn).await? {
                    User {
                        id: google_user.user_id,
                    }
                } else {
                    User::new(&mut conn).await?
                };
            let redirect = if UserData::from_user(user, &mut conn).await?.is_some() {
                Redirect::temporary("home")
            } else {
                Redirect::temporary("build-profile")
            };

            session.insert_or_update_for_user(user, &mut conn).await?;

            let session = models::UserSession::new_for_user(user, expires_at, &mut conn).await?;

            let mut cookie = Cookie::new(SESSION_COOKIE_NAME, session.token);

            cookie.set_secure(true);
            cookie.set_http_only(true);
            cookie.set_expires(OffsetDateTime::new_utc(
                expires_at.date(),
                expires_at.time(),
            ));
            cookie.set_path("/");
            cookie.set_secure(true);

            // Redirect here to the profile setup page. set cookie
            Ok(([(SET_COOKIE, cookie.encoded().to_string())], redirect))
        }
        AuthEndpoint::Twitch => todo!(),
    }
}

async fn attach(
    authentication: auth::Authentication,
    Path(endpoint): Path<AuthEndpoint>,
    Json(login_params): Json<LoginParams>,
) -> Result<(), Error> {
    match authentication {
        auth::Authentication::Unauthenticated => Err(Error::Custom {
            status_code: StatusCode::UNAUTHORIZED,
            error: "Requests to attach need to be made from an authenticated session".into(),
        }),
        auth::Authentication::Authenticated { user } => match endpoint {
            AuthEndpoint::Google => {
                let session = google::GoogleSession::from_code(
                    login_params.redirect_origin,
                    login_params.code,
                )
                .await?;

                let mut conn = POOL.get().await?;

                session.insert_or_update_for_user(user, &mut conn).await?;

                Ok(())
            }
            AuthEndpoint::Twitch => todo!(),
        },
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("An error occured: {status_code:?} => {error}")]
    Custom {
        status_code: StatusCode,
        error: String,
    },
    #[error("Failed to get connection from pool: {0:?}")]
    PoolError(#[from] diesel_async::pooled_connection::deadpool::PoolError),
    #[error("Failed to user using the token from the DB: {0:?}")]
    QueryError(#[from] diesel::result::Error),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::Custom { status_code, error } => (status_code, Html(error)).into_response(),
            Error::PoolError(_) | Error::QueryError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("{self:?}"))).into_response()
            }
        }
    }
}
