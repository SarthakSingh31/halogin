use diesel::{
    deserialize::Queryable, pg::Pg, upsert::excluded, AsChangeset, BoolExpressionMethods,
    ExpressionMethods, Insertable, JoinOnDsl, OptionalExtension, QueryDsl, Selectable,
    SelectableHelper,
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use fxhash::FxHashMap;
use image::{DynamicImage, ImageFormat};
use pgvector::Vector;
use reqwest::StatusCode;
use uuid::Uuid;

use crate::{
    storage::{Folder, Storage},
    Error,
};

use super::{schema, Encoder, User};

#[derive(Clone, Insertable, AsChangeset)]
#[diesel(table_name = schema::company)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CompanyInsertUpdate<'c> {
    full_name: &'c str,
    banner_desc: &'c str,
    logo_url: Option<&'c str>,
    embedding: Vector,
}

impl<'c> CompanyInsertUpdate<'c> {
    fn format_creator_descriptions(banner_desc: &str) -> String {
        format!("Question: Who are we?\nAnswer: {banner_desc}")
    }

    pub async fn insert(
        full_name: &str,
        banner_desc: &str,
        logo_hidden: Option<&str>,
        logo: Option<(DynamicImage, ImageFormat)>,
        conn: &mut impl AsyncConnection<Backend = Pg>,
        encoder: Encoder,
        storage: Storage,
    ) -> Result<Uuid, Error> {
        let embedding_desc = CompanyInsertUpdate::format_creator_descriptions(banner_desc);
        let embedding = encoder.encode(embedding_desc).await?;

        use schema::company::dsl as c_dsl;

        let company_id = diesel::insert_into(c_dsl::company)
            .values(&CompanyInsertUpdate {
                full_name,
                banner_desc,
                logo_url: None,
                embedding: embedding.into(),
            })
            .returning(c_dsl::id)
            .load(conn)
            .await?
            .pop()
            .expect("No company id was returned");

        let logo_path = storage
            .store_public_image(Folder::Logo, company_id, logo_hidden, logo)
            .await?;

        if let Some(logo_path) = logo_path {
            diesel::update(c_dsl::company)
                .set(c_dsl::logo_url.eq(logo_path))
                .filter(c_dsl::id.eq(company_id))
                .execute(conn)
                .await?;
        }

        Ok(company_id)
    }

    pub async fn update(
        company_id: Uuid,
        full_name: &str,
        banner_desc: &str,
        logo_hidden: Option<&str>,
        logo: Option<(DynamicImage, ImageFormat)>,
        conn: &mut impl AsyncConnection<Backend = Pg>,
        encoder: Encoder,
        storage: Storage,
    ) -> Result<(), Error> {
        let embedding_desc = CompanyInsertUpdate::format_creator_descriptions(banner_desc);
        let embedding = encoder.encode(embedding_desc).await?;

        use schema::company::dsl as c_dsl;

        let logo_path = storage
            .store_public_image(Folder::Logo, company_id, logo_hidden, logo)
            .await?;

        diesel::update(c_dsl::company)
            .set(&CompanyInsertUpdate {
                full_name,
                banner_desc,
                logo_url: logo_path.as_deref(),
                embedding: embedding.into(),
            })
            .filter(c_dsl::id.eq(company_id))
            .execute(conn)
            .await?;

        Ok(())
    }
}

#[derive(serde::Serialize)]
pub struct Company {
    pub id: Uuid,
    pub full_name: String,
    pub banner_desc: String,
    pub logo_url: String,
    pub users: FxHashMap<Uuid, CompanyUser>,
    pub invites: Vec<CompanyInvitationMinimal>,
}

impl Company {
    pub async fn list_for_user(
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Self>, Error> {
        use schema::company::dsl as c_dsl;
        use schema::companyuser::dsl as cu_dsl;

        let mut companies = Vec::default();

        let retrived_companies = cu_dsl::companyuser
            .filter(cu_dsl::user_id.eq(user.id))
            .inner_join(c_dsl::company.on(c_dsl::id.eq(cu_dsl::company_id)))
            .select((
                c_dsl::id,
                c_dsl::full_name,
                c_dsl::banner_desc,
                c_dsl::logo_url,
            ))
            .load::<(Uuid, String, String, String)>(conn)
            .await?;

        for (id, full_name, banner_desc, logo_url) in retrived_companies {
            companies.push(Company {
                id,
                full_name,
                banner_desc,
                logo_url,
                users: CompanyUser::list(id, conn).await?.into_iter().collect(),
                invites: CompanyInvitationMinimal::list(id, conn).await?,
            });
        }

        Ok(companies)
    }
}

#[derive(Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = schema::company)]
#[diesel(check_for_backend(diesel::pg::Pg))]

pub struct CompanyMinimal {
    pub id: Uuid,
    pub full_name: String,
    pub banner_desc: String,
    pub logo_url: String,
}

pub async fn users_in(
    company_id: Uuid,
    conn: &mut impl AsyncConnection<Backend = Pg>,
) -> Result<Vec<Uuid>, Error> {
    use schema::companyuser::dsl as cu_dsl;

    Ok(cu_dsl::companyuser
        .filter(cu_dsl::company_id.eq(company_id))
        .select(cu_dsl::user_id)
        .load(conn)
        .await?)
}

pub async fn delete(
    company_id: Uuid,
    conn: &mut impl AsyncConnection<Backend = Pg>,
) -> Result<(), Error> {
    use schema::company::dsl as c_dsl;

    diesel::delete(c_dsl::company)
        .filter(c_dsl::id.eq(company_id))
        .execute(conn)
        .await?;

    Ok(())
}

pub async fn add_user(
    company_id: Uuid,
    user: User,
    is_admin: bool,
    conn: &mut impl AsyncConnection<Backend = Pg>,
) -> Result<(), Error> {
    use schema::companyuser::dsl as cu_dsl;

    diesel::insert_into(cu_dsl::companyuser)
        .values((
            cu_dsl::company_id.eq(company_id),
            cu_dsl::user_id.eq(user.id),
            cu_dsl::is_admin.eq(is_admin),
        ))
        .execute(conn)
        .await?;

    Ok(())
}

#[derive(serde::Serialize)]
pub struct CompanyInvitationDetailed {
    from: UserProfile,
    company: CompanyMinimal,
    is_admin: bool,
}

impl CompanyInvitationDetailed {
    pub async fn list(
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Self>, Error> {
        use schema::company::dsl as c_dsl;
        use schema::companyuserinvitation::dsl as cui_dsl;
        use schema::companyuserprofile::dsl as cup_dsl;
        use schema::googleaccount::dsl as ga_dsl;

        let accounts = ga_dsl::googleaccount
            .filter(ga_dsl::user_id.eq(user.id))
            .inner_join(
                cui_dsl::companyuserinvitation.on(ga_dsl::email.eq(cui_dsl::invited_google_email)),
            )
            .inner_join(c_dsl::company.on(c_dsl::id.eq(cui_dsl::company_id)))
            .inner_join(cup_dsl::companyuserprofile.on(cup_dsl::user_id.eq(cui_dsl::from_user_id)))
            .select((
                UserProfile::as_select(),
                CompanyMinimal::as_select(),
                cui_dsl::will_be_given_admin,
            ))
            .load::<(UserProfile, CompanyMinimal, bool)>(conn)
            .await?
            .into_iter()
            .map(|(from, company, is_admin)| CompanyInvitationDetailed {
                from,
                company,
                is_admin,
            })
            .collect();

        Ok(accounts)
    }
}

#[derive(Debug, Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = schema::companyuserprofile)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserProfile {
    pub given_name: String,
    pub family_name: String,
    pub pronouns: String,
    pub pfp_path: String,
}

impl UserProfile {
    pub async fn insert_update(
        user: User,
        given_name: &str,
        family_name: &str,
        pronouns: &str,
        pfp_hidden: Option<&str>,
        pfp: Option<(DynamicImage, ImageFormat)>,
        conn: &mut impl AsyncConnection<Backend = Pg>,
        storage: Storage,
    ) -> Result<(), Error> {
        use schema::companyuserprofile::dsl as cup_dsl;

        let Some(pfp_path) = storage
            .store_public_image(Folder::ProfilePicture, user.id, pfp_hidden, pfp)
            .await?
        else {
            return Err(Error::Custom {
                status_code: StatusCode::BAD_REQUEST,
                error: "Missing pfp picture".into(),
            });
        };

        diesel::insert_into(cup_dsl::companyuserprofile)
            .values((
                cup_dsl::user_id.eq(user.id),
                cup_dsl::given_name.eq(given_name),
                cup_dsl::family_name.eq(family_name),
                cup_dsl::pronouns.eq(pronouns),
                cup_dsl::pfp_path.eq(pfp_path),
            ))
            .on_conflict(cup_dsl::user_id)
            .do_update()
            .set((
                cup_dsl::given_name.eq(excluded(cup_dsl::given_name)),
                cup_dsl::family_name.eq(excluded(cup_dsl::family_name)),
                cup_dsl::pronouns.eq(excluded(cup_dsl::pronouns)),
                cup_dsl::pfp_path.eq(excluded(cup_dsl::pfp_path)),
            ))
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn get(
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Option<Self>, Error> {
        use schema::companyuserprofile::dsl as cup_dsl;

        let profile = cup_dsl::companyuserprofile
            .filter(cup_dsl::user_id.eq(user.id))
            .select(UserProfile::as_select())
            .first(conn)
            .await
            .optional()?;

        Ok(profile)
    }
}

#[derive(serde::Serialize)]
pub struct CompanyInvitationMinimal {
    pub google_email: String,
    pub is_admin: bool,
    pub from_user: Uuid,
}

impl CompanyInvitationMinimal {
    pub async fn list(
        company_id: Uuid,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Self>, Error> {
        use schema::companyuserinvitation::dsl as cui_dsl;

        let mut invites = Vec::default();

        let recorded_invites = cui_dsl::companyuserinvitation
            .filter(cui_dsl::company_id.eq(company_id))
            .select((
                cui_dsl::invited_google_email,
                cui_dsl::will_be_given_admin,
                cui_dsl::from_user_id,
            ))
            .load::<(String, bool, Uuid)>(conn)
            .await?;

        for (google_email, is_admin, from_user) in recorded_invites {
            invites.push(CompanyInvitationMinimal {
                google_email,
                is_admin,
                from_user,
            });
        }

        Ok(invites)
    }
}

pub async fn invite_by_email(
    company_id: Uuid,
    google_email: String,
    is_admin: bool,
    from_user: User,
    conn: &mut impl AsyncConnection<Backend = Pg>,
) -> Result<(), Error> {
    use schema::companyuserinvitation::dsl as cui_dsl;

    diesel::insert_into(cui_dsl::companyuserinvitation)
        .values((
            cui_dsl::company_id.eq(company_id),
            cui_dsl::invited_google_email.eq(google_email),
            cui_dsl::will_be_given_admin.eq(is_admin),
            cui_dsl::from_user_id.eq(from_user.id),
        ))
        .execute(conn)
        .await?;

    Ok(())
}

pub async fn uninvite_by_email(
    company_id: Uuid,
    google_email: String,
    conn: &mut impl AsyncConnection<Backend = Pg>,
) -> Result<(), Error> {
    use schema::companyuserinvitation::dsl as cui_dsl;

    diesel::delete(cui_dsl::companyuserinvitation)
        .filter(
            cui_dsl::company_id
                .eq(company_id)
                .and(cui_dsl::invited_google_email.eq(google_email)),
        )
        .execute(conn)
        .await?;

    Ok(())
}

pub async fn accept_invitation(
    user: User,
    company_id: Uuid,
    conn: &mut impl AsyncConnection<Backend = Pg>,
) -> Result<(), Error> {
    use schema::companyuserinvitation::dsl as cui_dsl;
    use schema::googleaccount::dsl as ga_dsl;

    // TODO: Due to https://github.com/diesel-rs/diesel/issues/1478 cannot do in a single query

    let emails = ga_dsl::googleaccount
        .filter(ga_dsl::user_id.eq(user.id))
        .select(ga_dsl::email)
        .load::<String>(conn)
        .await?;

    let mut deleted_any_invites = false;
    let mut will_be_given_admin = false;
    for email in emails {
        let will_be_given_admins = diesel::delete(cui_dsl::companyuserinvitation)
            .filter(
                cui_dsl::company_id
                    .eq(company_id)
                    .and(cui_dsl::invited_google_email.eq(email)),
            )
            .returning(cui_dsl::will_be_given_admin)
            .load::<bool>(conn)
            .await?;
        if will_be_given_admins.len() > 0 {
            deleted_any_invites = true;
        }
        for permission in will_be_given_admins {
            will_be_given_admin |= permission;
        }
    }

    if !deleted_any_invites {
        return Err(Error::Custom {
            status_code: StatusCode::NOT_FOUND,
            error: "You don't have any invites from this company".into(),
        });
    }

    // TODO: A error in this query will erase the invitations without adding the user to the company
    use schema::companyuser::dsl as cu_dsl;
    diesel::insert_into(cu_dsl::companyuser)
        .values((
            cu_dsl::company_id.eq(company_id),
            cu_dsl::user_id.eq(user.id),
            cu_dsl::is_admin.eq(will_be_given_admin),
        ))
        .execute(conn)
        .await?;

    Ok(())
}

pub async fn reject_invitation(
    user: User,
    company_id: Uuid,
    conn: &mut impl AsyncConnection<Backend = Pg>,
) -> Result<(), Error> {
    use schema::companyuserinvitation::dsl as cui_dsl;
    use schema::googleaccount::dsl as ga_dsl;

    // TODO: Due to https://github.com/diesel-rs/diesel/issues/1478 cannot do in a single query

    let emails = ga_dsl::googleaccount
        .filter(ga_dsl::user_id.eq(user.id))
        .select(ga_dsl::email)
        .load::<String>(conn)
        .await?;

    let mut deleted_any_invites = false;
    for email in emails {
        let will_be_given_admins = diesel::delete(cui_dsl::companyuserinvitation)
            .filter(
                cui_dsl::company_id
                    .eq(company_id)
                    .and(cui_dsl::invited_google_email.eq(email)),
            )
            .returning(cui_dsl::will_be_given_admin)
            .load::<bool>(conn)
            .await?;
        if will_be_given_admins.len() > 0 {
            deleted_any_invites = true;
        }
    }

    if !deleted_any_invites {
        return Err(Error::Custom {
            status_code: StatusCode::NOT_FOUND,
            error: "You don't have any invites from this company".into(),
        });
    }

    Ok(())
}

pub async fn is_admin(
    company_id: Uuid,
    user: User,
    conn: &mut impl AsyncConnection<Backend = Pg>,
) -> Result<Option<bool>, Error> {
    use schema::companyuser::dsl as cu_dsl;

    let is_admin = cu_dsl::companyuser
        .filter(
            cu_dsl::company_id
                .eq(company_id)
                .and(cu_dsl::user_id.eq(user.id)),
        )
        .select(cu_dsl::is_admin)
        .first(conn)
        .await
        .optional()?;

    Ok(is_admin)
}

#[derive(serde::Serialize)]
pub struct CompanyUser {
    pub given_name: String,
    pub family_name: String,
    pub pronouns: String,
    pub pfp_path: String,
    pub is_admin: bool,
}

impl CompanyUser {
    pub async fn list(
        company_id: Uuid,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<impl Iterator<Item = (Uuid, Self)>, Error> {
        use schema::companyuser::dsl as cu_dsl;
        use schema::companyuserprofile::dsl as cup_dsl;

        Ok(cu_dsl::companyuser
            .filter(cu_dsl::company_id.eq(company_id))
            .inner_join(cup_dsl::companyuserprofile.on(cu_dsl::user_id.eq(cup_dsl::user_id)))
            .select((
                cup_dsl::user_id,
                cup_dsl::given_name,
                cup_dsl::family_name,
                cup_dsl::pronouns,
                cup_dsl::pfp_path,
                cu_dsl::is_admin,
            ))
            .load::<(Uuid, String, String, String, String, bool)>(conn)
            .await?
            .into_iter()
            .map(
                |(id, given_name, family_name, pronouns, pfp_path, is_admin)| {
                    (
                        id,
                        CompanyUser {
                            given_name,
                            family_name,
                            pronouns,
                            pfp_path,
                            is_admin,
                        },
                    )
                },
            ))
    }
}
