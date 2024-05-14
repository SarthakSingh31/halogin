use axum::{
    extract::{Multipart, Path},
    http::StatusCode,
    routing, Json, Router,
};
use fxhash::FxHashMap;
use uuid::Uuid;

use crate::{
    db::{company, Encoder, User},
    state::DbConn,
    storage::Storage,
    utils::formdata::ImageFileBuilder,
    Error,
};

const PROFILE_FIELDS: &'static [&'static str] = &["given_name", "family_name", "pronouns"];
const COMPANY_FIELDS: &'static [&'static str] = &["full_name", "banner_desc"];

async fn list_users(
    user: User,
    DbConn { mut conn }: DbConn,
    Path(company_id): Path<Uuid>,
) -> Result<Json<FxHashMap<Uuid, company::CompanyUser>>, Error> {
    if !company::is_admin(company_id, user, &mut conn)
        .await?
        .unwrap_or(false)
    {
        return Err(Error::Custom {
            status_code: StatusCode::UNAUTHORIZED,
            error: "You are not an admin of this company".into(),
        });
    }

    let users = company::CompanyUser::list(company_id, &mut conn).await?;

    Ok(Json(users.collect()))
}

async fn insert_update_user_profile(
    user: User,
    DbConn { mut conn }: DbConn,
    storage: Storage,
    multipart: Multipart,
) -> Result<(), Error> {
    let builder = ImageFileBuilder::build(multipart).await?;

    let missing_fields = builder.missing_fields(&PROFILE_FIELDS);
    if missing_fields.is_empty() {
        company::UserProfile::insert_update(
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

async fn get_user_profile(
    user: User,
    DbConn { mut conn }: DbConn,
) -> Result<Json<company::UserProfile>, Error> {
    match company::UserProfile::get(user, &mut conn).await? {
        Some(profile) => Ok(Json(profile)),
        None => Err(Error::Custom {
            status_code: StatusCode::NOT_FOUND,
            error: "No company user profile found for this user".into(),
        }),
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
        let company_id = company::CompanyInsertUpdate::insert(
            &builder.fields[COMPANY_FIELDS[0]],
            &builder.fields[COMPANY_FIELDS[1]],
            builder.fields.get("logo_hidden").map(|s| s.as_str()),
            builder.image,
            &mut conn,
            encoder,
            storage,
        )
        .await?;

        if let Err(err) = company::add_user(company_id, user, true, &mut conn).await {
            company::delete(company_id, &mut conn).await?;

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
    if !company::is_admin(company_id, user, &mut conn)
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
        company::CompanyInsertUpdate::update(
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

async fn get_companies(
    user: User,
    DbConn { mut conn }: DbConn,
) -> Result<Json<Vec<company::Company>>, Error> {
    company::Company::list_for_user(user, &mut conn)
        .await
        .map(Json)
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
    if !company::is_admin(req.company_id, user, &mut conn)
        .await?
        .unwrap_or(false)
    {
        return Err(Error::Custom {
            status_code: StatusCode::UNAUTHORIZED,
            error: "You are not an admin of this company".into(),
        });
    }

    company::invite_by_email(
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
    if !company::is_admin(req.company_id, user, &mut conn)
        .await?
        .unwrap_or(false)
    {
        return Err(Error::Custom {
            status_code: StatusCode::UNAUTHORIZED,
            error: "You are not an admin of this company".into(),
        });
    }

    company::uninvite_by_email(req.company_id, req.google_email, &mut conn).await?;

    Ok(())
}

async fn get_invites(
    user: User,
    DbConn { mut conn }: DbConn,
) -> Result<Json<Vec<company::CompanyInvitationDetailed>>, Error> {
    company::CompanyInvitationDetailed::list(user, &mut conn)
        .await
        .map(Json)
}

async fn accept_invitation(
    user: User,
    DbConn { mut conn }: DbConn,
    Path(company_id): Path<Uuid>,
) -> Result<(), Error> {
    company::accept_invitation(user, company_id, &mut conn).await
}

async fn reject_invitation(
    user: User,
    DbConn { mut conn }: DbConn,
    Path(company_id): Path<Uuid>,
) -> Result<(), Error> {
    company::reject_invitation(user, company_id, &mut conn).await
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new()
        .route("/", routing::get(get_companies).post(insert_company))
        .route("/:company-id", routing::patch(update_company))
        .route("/:company-id/user", routing::get(list_users))
        .route(
            "/:company-id/invite",
            routing::post(invite_user_to_company).delete(uninvite_user_to_company),
        )
        .route(
            "/:company-id/invite/accept",
            routing::get(accept_invitation),
        )
        .route(
            "/:company-id/invite/reject",
            routing::get(reject_invitation),
        )
        .route(
            "/user-profile",
            routing::get(get_user_profile).post(insert_update_user_profile),
        )
        .route("/invite", routing::get(get_invites))
}
