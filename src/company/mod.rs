use axum::{
    extract::{Multipart, Path},
    http::StatusCode,
    routing, Json, Router,
};
use uuid::Uuid;

use crate::{
    db::{Company, Encoder, User},
    state::DbConn,
    storage::Storage,
    utils::formdata::ImageFileBuilder,
    Error,
};

const PROFILE_FIELDS: &'static [&'static str] = &["given_name", "family_name", "pronouns"];
const COMPANY_FIELDS: &'static [&'static str] = &["full_name", "banner_desc"];

async fn insert_update_profile(
    user: User,
    DbConn { mut conn }: DbConn,
    storage: Storage,
    multipart: Multipart,
) -> Result<(), Error> {
    let builder = ImageFileBuilder::build(multipart).await?;

    let missing_fields = builder.missing_fields(&PROFILE_FIELDS);
    if missing_fields.is_empty() {
        Company::insert_update_user_profile(
            user,
            &builder.fields[PROFILE_FIELDS[0]],
            &builder.fields[PROFILE_FIELDS[1]],
            &builder.fields[PROFILE_FIELDS[1]],
            builder.fields.get("pfp_hidden").map(|s| s.as_str()),
            builder.image,
            &mut conn,
            storage,
        )
        .await?;

        Ok(())
    } else {
        Err(Error::Custom {
            status_code: StatusCode::BAD_REQUEST,
            error: format!("Missing fields: {missing_fields:?}"),
        })
    }
}

#[derive(serde::Serialize)]
struct InsertResponse {
    company_id: Uuid,
}

async fn insert_company(
    user: User,
    DbConn { mut conn }: DbConn,
    encoder: Encoder,
    storage: Storage,
    multipart: Multipart,
) -> Result<Json<InsertResponse>, Error> {
    let builder = ImageFileBuilder::build(multipart).await?;

    let missing_fields = builder.missing_fields(&COMPANY_FIELDS);
    if missing_fields.is_empty() {
        let company_id = Company::insert(
            &builder.fields[COMPANY_FIELDS[0]],
            &builder.fields[COMPANY_FIELDS[1]],
            builder.fields.get("logo_hidden").map(|s| s.as_str()),
            builder.image,
            &mut conn,
            encoder,
            storage,
        )
        .await?;

        if let Err(err) = Company::add_user(company_id, user, true, &mut conn).await {
            Company::delete(company_id, &mut conn).await?;

            return Err(err);
        }

        Ok(Json(InsertResponse { company_id }))
    } else {
        Err(Error::Custom {
            status_code: StatusCode::BAD_REQUEST,
            error: format!("Missing fields: {missing_fields:?}"),
        })
    }
}

async fn update_company(
    user: User,
    DbConn { mut conn }: DbConn,
    Path(company_id): Path<Uuid>,
    encoder: Encoder,
    storage: Storage,
    multipart: Multipart,
) -> Result<(), Error> {
    if !Company::is_admin(company_id, user, &mut conn)
        .await?
        .unwrap_or(false)
    {
        return Err(Error::Custom {
            status_code: StatusCode::UNAUTHORIZED,
            error: "You are not an admin of this company".into(),
        });
    }

    let builder = ImageFileBuilder::build(multipart).await?;

    let missing_fields = builder.missing_fields(&COMPANY_FIELDS);
    if missing_fields.is_empty() {
        Company::update(
            company_id,
            &builder.fields[COMPANY_FIELDS[0]],
            &builder.fields[COMPANY_FIELDS[1]],
            builder.fields.get("logo_hidden").map(|s| s.as_str()),
            builder.image,
            &mut conn,
            encoder,
            storage,
        )
        .await?;

        Ok(())
    } else {
        Err(Error::Custom {
            status_code: StatusCode::BAD_REQUEST,
            error: format!("Missing fields: {missing_fields:?}"),
        })
    }
}

#[derive(serde::Deserialize)]
struct InviteRequest {
    company_id: Uuid,
    google_email: String,
    is_admin: bool,
}

async fn invite_user_to_company(
    user: User,
    DbConn { mut conn }: DbConn,
    Json(req): Json<InviteRequest>,
) -> Result<(), Error> {
    if !Company::is_admin(req.company_id, user, &mut conn)
        .await?
        .unwrap_or(false)
    {
        return Err(Error::Custom {
            status_code: StatusCode::UNAUTHORIZED,
            error: "You are not an admin of this company".into(),
        });
    }

    Company::invite_by_email(
        req.company_id,
        req.google_email,
        req.is_admin,
        user,
        &mut conn,
    )
    .await?;

    Ok(())
}

#[derive(serde::Deserialize)]
struct UninviteRequest {
    company_id: Uuid,
    google_email: String,
}

async fn uninvite_user_to_company(
    user: User,
    DbConn { mut conn }: DbConn,
    Json(req): Json<UninviteRequest>,
) -> Result<(), Error> {
    if !Company::is_admin(req.company_id, user, &mut conn)
        .await?
        .unwrap_or(false)
    {
        return Err(Error::Custom {
            status_code: StatusCode::UNAUTHORIZED,
            error: "You are not an admin of this company".into(),
        });
    }

    Company::uninvite_by_email(req.company_id, req.google_email, &mut conn).await?;

    Ok(())
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new()
        .route("/", routing::post(insert_company))
        .route("/user-profile", routing::post(insert_update_profile))
        .route("/:company-id", routing::patch(update_company))
        .route(
            "/:company-id/invite",
            routing::post(invite_user_to_company).delete(uninvite_user_to_company),
        )
}
