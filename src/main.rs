#![feature(lazy_cell)]

mod auth;
mod google;
mod models;
mod schema;

use std::sync::LazyLock;

use axum::{extract::Path, routing, Json, Router};
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};
use serde::Deserialize;

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

    let app = Router::new()
        // `GET /` goes to `root`
        .route("/login/:endpoint", routing::post(login))
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

async fn login(Path(endpoint): Path<AuthEndpoint>, Json(login_params): Json<LoginParams>) {
    match endpoint {
        AuthEndpoint::Google => {
            let session =
                google::GoogleSession::from_code(login_params.redirect_origin, login_params.code)
                    .await;

            dbg!(session);
        }
        AuthEndpoint::Twitch => todo!(),
    }
}
