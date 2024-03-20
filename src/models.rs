use diesel::{pg::Pg, prelude::*};
use diesel_async::{AsyncConnection, RunQueryDsl};
use rand::Rng;
use time::{OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

use crate::Error;

#[derive(Clone, Copy, Insertable, Queryable)]
#[diesel(table_name = crate::schema::inneruser)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: Uuid,
}

impl User {
    pub async fn new(conn: &mut impl AsyncConnection<Backend = Pg>) -> Result<Self, Error> {
        let user = User { id: Uuid::new_v4() };

        use crate::schema::inneruser::dsl as dsl_iu;

        diesel::insert_into(dsl_iu::inneruser)
            .values(user)
            .execute(conn)
            .await?;

        Ok(user)
    }
}

#[derive(Insertable, Queryable)]
#[diesel(table_name = crate::schema::inneruserdata)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserData {
    pub given_name: String,
    pub family_name: String,
    pub banner_desc: String,
}

impl UserData {
    pub async fn from_user(
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Option<Self>, Error> {
        use crate::schema::inneruserdata::dsl as dsl_iud;

        let user = dsl_iud::inneruserdata
            .select((
                dsl_iud::given_name,
                dsl_iud::family_name,
                dsl_iud::banner_desc,
            ))
            .filter(dsl_iud::id.eq(user.id))
            .first(conn)
            .await
            .optional()?;

        Ok(user)
    }
}

#[derive(Clone, Insertable, Queryable)]
#[diesel(table_name = crate::schema::innerusersession)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserSession {
    pub token: String,
    pub expires_at: PrimitiveDateTime,
    pub user_id: Uuid,
}

impl UserSession {
    pub async fn new_for_user(
        user: User,
        expires_at: PrimitiveDateTime,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Self, Error> {
        let token = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();
        let session = UserSession {
            token,
            expires_at,
            user_id: user.id,
        };

        diesel::insert_into(crate::schema::innerusersession::dsl::innerusersession)
            .values(session.clone())
            .execute(conn)
            .await?;

        Ok(session)
    }

    pub async fn get_user_by_token(
        token: &str,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Option<User>, Error> {
        use crate::schema::innerusersession::dsl as dsl_ius;

        let user = dsl_ius::innerusersession
            .select((dsl_ius::user_id,))
            .filter(dsl_ius::token.eq(token))
            .first(conn)
            .await
            .optional()?;

        Ok(user)
    }

    pub async fn prune_expired(conn: &mut impl AsyncConnection<Backend = Pg>) -> Result<(), Error> {
        let now = OffsetDateTime::now_utc();
        let now = PrimitiveDateTime::new(now.date(), now.time());

        use crate::schema::innerusersession::dsl as dsl_ius;

        diesel::delete(dsl_ius::innerusersession)
            .filter(dsl_ius::expires_at.lt(now))
            .execute(conn)
            .await?;

        Ok(())
    }
}

#[derive(Insertable, Queryable)]
#[diesel(table_name = crate::schema::twitchuser)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TwitchUser {
    pub id: String,
    pub access_token: String,
    pub expires_at: PrimitiveDateTime,
    pub refresh_token: String,
    pub user_id: Uuid,
}

#[derive(Insertable, Queryable)]
#[diesel(table_name = crate::schema::googleuser)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GoogleUser {
    pub sub: String,
    pub access_token: String,
    pub expires_at: PrimitiveDateTime,
    pub refresh_token: String,
    pub user_id: Uuid,
}

impl GoogleUser {
    pub async fn from_sub(
        sub: &str,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Option<Self>, Error> {
        use crate::schema::googleuser::dsl as dsl_gu;

        let user = dsl_gu::googleuser
            .filter(dsl_gu::sub.eq(sub))
            .first(conn)
            .await
            .optional()?;

        Ok(user)
    }
}
