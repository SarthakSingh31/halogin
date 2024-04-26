use axum::{extract::Multipart, http::StatusCode, routing, Json, Router};

use crate::{
    db::{CreatorProfileInsert, CreatorProfileQuery, Encoder, User},
    state::DbConn,
    storage::Storage,
    utils::formdata::ImageFileBuilder,
    Error,
};

const PROFILE_FIELDS: &'static [&'static str] = &[
    "given_name",
    "family_name",
    "pronouns",
    "profile_desc",
    "content_desc",
    "audience_desc",
];

async fn insert_update_profile(
    user: User,
    DbConn { mut conn }: DbConn,
    encoder: Encoder,
    storage: Storage,
    multipart: Multipart,
) -> Result<(StatusCode, String), Error> {
    let builder = ImageFileBuilder::build(multipart).await?;

    let missing_fields = builder.missing_fields(&PROFILE_FIELDS);
    if missing_fields.is_empty() {
        CreatorProfileInsert::insert_update(
            user,
            &builder.fields[PROFILE_FIELDS[0]],
            &builder.fields[PROFILE_FIELDS[1]],
            &builder.fields[PROFILE_FIELDS[2]],
            &builder.fields[PROFILE_FIELDS[3]],
            &builder.fields[PROFILE_FIELDS[4]],
            &builder.fields[PROFILE_FIELDS[5]],
            builder.fields.get("pfp_hidden").map(|s| s.as_str()),
            builder.image,
            &mut conn,
            encoder,
            storage,
        )
        .await?;

        return Ok((StatusCode::OK, "OK".into()));
    }

    Err(Error::Custom {
        status_code: StatusCode::BAD_REQUEST,
        error: format!("Missing fields: {missing_fields:?}"),
    })
}

async fn get_profile(
    user: User,
    DbConn { mut conn }: DbConn,
) -> Result<Json<CreatorProfileQuery>, Error> {
    if let Some(profile) = CreatorProfileQuery::get(user, &mut conn).await? {
        return Ok(Json(profile));
    } else {
        Err(Error::Custom {
            status_code: StatusCode::NOT_FOUND,
            error: "There is no creator profile informatio for you".into(),
        })
    }
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new().route(
        "/profile",
        routing::get(get_profile).post(insert_update_profile),
    )
}
