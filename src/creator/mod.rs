use axum::{extract::Multipart, http::StatusCode, routing, Json, Router};
use fxhash::FxHashMap;
use image::{DynamicImage, ImageFormat};

use crate::{
    db::{CreatorData, Encoder, GoogleAccount, TwitchAccount, User},
    state::DbConn,
    storage::Storage,
    utils::AuthenticationHeader,
    Error,
};

pub async fn insert_update_data(
    user: User,
    DbConn { mut conn }: DbConn,
    encoder: Encoder,
    storage: Storage,
    mut multipart: Multipart,
) -> Result<(StatusCode, String), Error> {
    #[derive(Default)]
    struct Builder {
        fields: FxHashMap<String, String>,
        pfp: Option<(DynamicImage, ImageFormat)>,
    }

    impl Builder {
        const FIELDS: &'static [&'static str] = &[
            "given_name",
            "family_name",
            "pronouns",
            "profile_desc",
            "content_desc",
            "audience_desc",
        ];

        fn missing_fields(&self) -> Vec<&'static str> {
            let mut missing = Vec::default();
            for needed in Self::FIELDS {
                if !self.fields.contains_key(*needed) {
                    missing.push(*needed);
                }
            }
            missing
        }
    }

    let mut builder = Builder::default();

    while let Some(field) = multipart.next_field().await? {
        if let Some(file_name) = field.file_name() {
            let (_name, ext) = file_name.split_once(".").ok_or(Error::Custom {
                status_code: StatusCode::BAD_REQUEST,
                error: format!("File name: {file_name} has no extension"),
            })?;
            let format = ImageFormat::from_extension(ext).ok_or(Error::Custom {
                status_code: StatusCode::BAD_REQUEST,
                error: format!("Could not figure out image format from extension: {ext}"),
            })?;

            let img_bytes = field.bytes().await?.to_vec();
            let image = image::load_from_memory_with_format(&img_bytes, format)?;

            builder.pfp = Some((image, format));
        } else if let Some(name) = field.name() {
            builder.fields.insert(name.into(), field.text().await?);
        }
    }

    if builder.missing_fields().is_empty() {
        CreatorData::insert_update(
            user,
            &builder.fields[Builder::FIELDS[0]],
            &builder.fields[Builder::FIELDS[1]],
            &builder.fields[Builder::FIELDS[2]],
            &builder.fields[Builder::FIELDS[3]],
            &builder.fields[Builder::FIELDS[4]],
            &builder.fields[Builder::FIELDS[5]],
            builder.fields.get("pfp_hidden").map(|s| s.as_str()),
            builder.pfp,
            &mut conn,
            encoder,
            storage,
        )
        .await?;

        return Ok((StatusCode::OK, "OK".into()));
    }

    Err(Error::Custom {
        status_code: StatusCode::BAD_REQUEST,
        error: format!("Missing fields: {:?}", builder.missing_fields()),
    })
}

pub async fn account_pfps(
    user: User,
    DbConn { mut conn }: DbConn,
) -> Result<Json<Vec<String>>, Error> {
    let mut pfps = Vec::default();

    let client = reqwest::Client::new();

    #[derive(serde::Deserialize)]
    struct GoogleResp {
        items: Vec<GoogleItem>,
    }
    #[derive(serde::Deserialize)]
    struct GoogleItem {
        snippet: GoogleSnippet,
    }
    #[derive(serde::Deserialize)]
    struct GoogleSnippet {
        thumbnails: GoogleThumbnails,
    }
    #[derive(serde::Deserialize)]
    struct GoogleThumbnails {
        high: GoogleImageData,
    }
    #[derive(serde::Deserialize)]
    struct GoogleImageData {
        url: String,
    }

    let google_accounts = GoogleAccount::list(user, &mut conn).await?;
    for mut account in google_accounts {
        let req = client.get("https://www.googleapis.com/youtube/v3/channels?part=snippet&mine=true&fields=items%2Fsnippet%2Fthumbnails")
            .headers(account.authentication_header(&mut conn).await?)
            .build()?;
        let resp: GoogleResp = client.execute(req).await?.json().await?;
        for item in resp.items {
            pfps.push(item.snippet.thumbnails.high.url);
        }
    }

    #[derive(serde::Deserialize)]
    struct TwitchResp {
        data: Vec<TwitchUser>,
    }
    #[derive(serde::Deserialize)]
    struct TwitchUser {
        profile_image_url: String,
    }

    let twitch_accounts = TwitchAccount::list(user, &mut conn).await?;
    for mut account in twitch_accounts {
        let req = client
            .get("https://api.twitch.tv/helix/users")
            .headers(account.authentication_header(&mut conn).await?)
            .build()?;
        let resp: TwitchResp = client.execute(req).await?.json().await?;
        for user in resp.data {
            pfps.push(user.profile_image_url);
        }
    }

    Ok(Json(pfps))
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new()
        .route("/data", routing::post(insert_update_data))
        .route("/account_pfps", routing::get(account_pfps))
}
