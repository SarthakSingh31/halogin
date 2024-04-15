use axum::{
    body::Body,
    extract::{FromRequestParts, Path},
    http::{header, request::Parts, StatusCode},
    response::IntoResponse,
    routing, Router,
};
use image::{DynamicImage, ImageFormat};
use tokio::fs;
use tokio_util::io::ReaderStream;

use crate::{
    db::User,
    state::{AppState, Config},
};

type Result<T> = std::result::Result<T, crate::Error>;

#[derive(Clone, Copy)]
pub struct Storage {
    config: Config,
}

impl Storage {
    const THUMBNAIL_IMG_WIDTH: u32 = 400;
    const THUMBNAIL_IMG_HEIGHT: u32 = 400;

    pub async fn store_public_pfp(
        &self,
        user: User,
        pfp: DynamicImage,
        format: ImageFormat,
    ) -> Result<String> {
        let uuid = user.id.to_string();
        let folder_id = uuid.chars().next().expect("User Id has not chars");

        let mut path = self.config.storage_path.to_path_buf();
        path.push("pfp");
        path.push(folder_id.to_ascii_lowercase().to_string());

        fs::create_dir_all(&path).await?;

        let thumbnail = pfp.thumbnail(Self::THUMBNAIL_IMG_WIDTH, Self::THUMBNAIL_IMG_HEIGHT);

        path.push(format!("{uuid}.{}", format.extensions_str()[0]));

        {
            let path = path.clone();
            tokio::task::spawn_blocking(move || thumbnail.save_with_format(&path, format))
                .await??;
        }

        Ok(format!("static/pfp/{uuid}.{}", format.extensions_str()[0]))
    }

    async fn get_public_pfp(Path(name): Path<String>, config: Config) -> impl IntoResponse {
        let mut path = config.storage_path.to_path_buf();
        path.push("pfp");

        let folder_id = name.chars().next().expect("User Id has not chars");
        path.push(folder_id.to_ascii_lowercase().to_string());

        path.push(&name);

        let file = match tokio::fs::File::open(&path).await {
            Ok(file) => file,
            Err(err) => return Err((StatusCode::NOT_FOUND, format!("File not found: {}", err))),
        };
        let content_type = match mime_guess::from_path(&path).first_raw() {
            Some(mime) => mime,
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "MIME Type couldn't be determined".to_string(),
                ))
            }
        };

        // convert the `AsyncRead` into a `Stream`
        let stream = ReaderStream::new(file);
        // convert the `Stream` into an `axum::body::HttpBody`
        let body = Body::from_stream(stream);

        let headers = [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{:?}\"", name),
            ),
        ];

        Ok((headers, body))
    }
}

#[axum::async_trait]
impl FromRequestParts<AppState> for Storage {
    type Rejection = crate::Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> std::result::Result<Self, Self::Rejection> {
        Ok(Storage {
            config: state.config(),
        })
    }
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new().route("/static/pfp/:name", routing::get(Storage::get_public_pfp))
}
