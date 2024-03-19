use diesel::prelude::*;
use time::PrimitiveDateTime;
use uuid::Uuid;

#[derive(Insertable, Queryable)]
#[diesel(table_name = crate::schema::inneruser)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: Uuid,
}

#[derive(Insertable, Queryable)]
#[diesel(table_name = crate::schema::inneruserdata)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserData {
    pub given_name: String,
    pub family_name: String,
    pub banner_desc: String,
}

#[derive(Insertable, Queryable)]
#[diesel(table_name = crate::schema::innerusersession)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserSession {
    pub token: String,
    pub user_id: Uuid,
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
