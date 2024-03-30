use std::io::Write;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::COOKIE, request::Parts},
};
use diesel::{
    data_types::Cents,
    deserialize::{self, FromSql, FromSqlRow},
    pg::{Pg, PgValue},
    prelude::*,
    serialize::{self, IsNull, Output, ToSql},
    AsExpression,
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use oauth2::RefreshToken;
use rand::Rng;
use time::{Duration, OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

use crate::{google::GoogleSession, utils::oauth::OAuthAccountHelper, Error, SESSION_COOKIE_NAME};
use crate::{twitch::TwitchSession, AppState};
const BUFFER_TIME: Duration = Duration::seconds(1);

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

#[async_trait]
impl FromRequestParts<AppState> for User {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(cookies) = parts.headers.get(COOKIE) {
            let parts = cookies.as_bytes().split(|c| *c == b';');
            for part in parts {
                if let Ok(part) = std::str::from_utf8(part) {
                    let part = part.trim();

                    if let Some((name, value)) = part.split_once('=') {
                        if name == SESSION_COOKIE_NAME {
                            let mut conn = state.get_conn().await?;

                            // We ignore the session cookie if we cannot find a session associated with it
                            if let Some(user) =
                                UserSession::get_user_by_token(value, &mut conn).await?
                            {
                                return Ok(user);
                            }
                        }
                    }
                }
            }
        }

        Err(Error::Unauthorized)
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
            .take(256)
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

        let now = OffsetDateTime::now_utc();
        let now = PrimitiveDateTime::new(now.date(), now.time());

        let user = dsl_ius::innerusersession
            .select((dsl_ius::user_id,))
            .filter(dsl_ius::token.eq(token))
            .filter(dsl_ius::expires_at.gt(now))
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
#[diesel(table_name = crate::schema::twitchaccount)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TwitchAccount {
    pub id: String,
    pub access_token: String,
    pub expires_at: PrimitiveDateTime,
    pub refresh_token: String,
    pub user_id: Uuid,
}

impl TwitchAccount {
    pub fn meta(&self) -> TwitchAccountMeta {
        TwitchAccountMeta {
            id: self.id.clone(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchAccountMeta {
    pub id: String,
}

#[derive(Clone, Insertable, Queryable, AsChangeset)]
#[diesel(table_name = crate::schema::googleaccount)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GoogleAccount {
    pub sub: String,
    pub email: String,
    pub access_token: String,
    pub expires_at: PrimitiveDateTime,
    pub refresh_token: String,
    pub user_id: Uuid,
}

impl GoogleAccount {
    pub async fn from_sub(
        sub: &str,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Option<Self>, Error> {
        use crate::schema::googleaccount::dsl as dsl_ga;

        let user = dsl_ga::googleaccount
            .filter(dsl_ga::sub.eq(sub))
            .first(conn)
            .await
            .optional()?;

        Ok(user)
    }

    pub async fn list(
        user: User,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Self>, Error> {
        use crate::schema::googleaccount::dsl as dsl_ga;

        let accounts = dsl_ga::googleaccount
            .filter(dsl_ga::user_id.eq(user.id))
            .load(conn)
            .await?;

        Ok(accounts)
    }

    pub fn meta(&self) -> GoogleAccountMeta {
        GoogleAccountMeta {
            sub: self.sub.clone(),
            email: self.email.clone(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAccountMeta {
    pub sub: String,
    pub email: String,
}

pub trait AuthenticationHeader {
    type Session: OAuthAccountHelper;

    fn access_token(&self) -> &str;
    fn expires_at(&self) -> PrimitiveDateTime;
    fn refresh_token(&self) -> String;
    fn user(&self) -> User;
    fn update(&mut self, session: Self::Session);

    async fn authentication_header(
        &mut self,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<reqwest::header::HeaderMap, Error> {
        let now = OffsetDateTime::now_utc();
        if (PrimitiveDateTime::new(now.date(), now.time()) + BUFFER_TIME) > self.expires_at() {
            let session = Self::Session::renew(RefreshToken::new(self.refresh_token())).await?;

            session.insert_or_update_for_user(self.user(), conn).await?;

            self.update(session);
        }

        let mut map = reqwest::header::HeaderMap::new();
        map.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.access_token()))
                .expect("Failed to make the bearer token header value"),
        );

        Ok(map)
    }
}

impl AuthenticationHeader for GoogleAccount {
    type Session = GoogleSession;

    fn access_token(&self) -> &str {
        &self.access_token
    }

    fn expires_at(&self) -> PrimitiveDateTime {
        self.expires_at
    }

    fn refresh_token(&self) -> String {
        self.refresh_token.clone()
    }

    fn user(&self) -> User {
        User { id: self.user_id }
    }

    fn update(&mut self, session: Self::Session) {
        self.access_token = session.access_token();
        self.expires_at = session.expires_at();
        self.refresh_token = session.refresh_token();
        self.email = session.email();
        // session.sub does not change so we don't need to update it
    }
}

impl AuthenticationHeader for TwitchAccount {
    type Session = TwitchSession;

    fn access_token(&self) -> &str {
        &self.access_token
    }

    fn expires_at(&self) -> PrimitiveDateTime {
        self.expires_at
    }

    fn refresh_token(&self) -> String {
        self.refresh_token.clone()
    }

    fn user(&self) -> User {
        User { id: self.user_id }
    }

    fn update(&mut self, session: Self::Session) {
        self.access_token = session.access_token();
        self.expires_at = session.expires_at();
        self.refresh_token = session.refresh_token();
        // session.id does not change so we don't need to update it
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    FromSqlRow,
    AsExpression,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[diesel(sql_type = crate::schema::sql_types::Contractstatus)]
pub enum ContractStatus {
    AcceptedByCreator,
    WithdrawnByCompany,
    CancelledByCreator,
    FinishedByCreator,
    ApprovedByCompany,
}

impl ToSql<crate::schema::sql_types::Contractstatus, Pg> for ContractStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            ContractStatus::AcceptedByCreator => out.write_all(b"AcceptedByCreator")?,
            ContractStatus::WithdrawnByCompany => out.write_all(b"WithdrawnByCompany")?,
            ContractStatus::CancelledByCreator => out.write_all(b"CancelledByCreator")?,
            ContractStatus::FinishedByCreator => out.write_all(b"FinishedByCreator")?,
            ContractStatus::ApprovedByCompany => out.write_all(b"ApprovedByCompany")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<crate::schema::sql_types::Contractstatus, Pg> for ContractStatus {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"AcceptedByCreator" => Ok(ContractStatus::AcceptedByCreator),
            b"WithdrawnByCompany" => Ok(ContractStatus::WithdrawnByCompany),
            b"CancelledByCreator" => Ok(ContractStatus::CancelledByCreator),
            b"FinishedByCreator" => Ok(ContractStatus::FinishedByCreator),
            b"ApprovedByCompany" => Ok(ContractStatus::ApprovedByCompany),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[derive(Clone, Insertable, Queryable, AsChangeset)]
#[diesel(table_name = crate::schema::companyuser)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CompanyUser {
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub is_admin: bool,
}

impl CompanyUser {
    pub async fn company_for_user(
        user_id: Uuid,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<CompanyInfo>, Error> {
        use crate::schema::companyuser::dsl as dsl_cu;

        let company_ids = dsl_cu::companyuser
            .filter(dsl_cu::user_id.eq(user_id))
            .select(dsl_cu::company_id)
            .load::<Uuid>(conn)
            .await?;

        let mut companies = Vec::default();

        use crate::schema::company::dsl as dsl_c;

        for company_id in company_ids {
            let (name, logo) = dsl_c::company
                .filter(dsl_c::id.eq(company_id))
                .select((dsl_c::full_name, dsl_c::logo_url))
                .first::<(String, String)>(conn)
                .await?;
            companies.push(CompanyInfo {
                id: company_id,
                name,
                logo,
            });
        }

        Ok(companies)
    }

    pub async fn users_in_company(
        company_id: Uuid,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Uuid>, Error> {
        use crate::schema::companyuser::dsl as dsl_cu;

        let users = dsl_cu::companyuser
            .filter(dsl_cu::company_id.eq(company_id))
            .select(dsl_cu::user_id)
            .load::<Uuid>(conn)
            .await?;

        Ok(users)
    }
}

#[derive(Clone, Insertable, Queryable, AsChangeset)]
#[diesel(table_name = crate::schema::chatroom)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChatRoom {
    pub id: Uuid,
    pub company_id: Uuid,
    pub user_id: Uuid,
}

impl ChatRoom {
    pub async fn from_id(
        room_id: Uuid,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Option<Self>, Error> {
        use crate::schema::chatroom::dsl as dsl_cr;

        let room = dsl_cr::chatroom
            .filter(dsl_cr::id.eq(room_id))
            .first(conn)
            .await
            .optional()?;

        Ok(room)
    }

    pub async fn create(
        company_id: Uuid,
        user_id: Uuid,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Uuid, Error> {
        use crate::schema::chatroom::dsl as dsl_cr;

        let room_id = Uuid::new_v4();

        diesel::insert_into(dsl_cr::chatroom)
            .values((
                dsl_cr::id.eq(room_id),
                dsl_cr::company_id.eq(company_id),
                dsl_cr::user_id.eq(user_id),
            ))
            .execute(conn)
            .await?;

        Ok(room_id)
    }

    pub async fn list(
        user_id: Uuid,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Uuid>, Error> {
        use crate::schema::chatroom::dsl as dsl_cr;

        let rooms = dsl_cr::chatroom
            .filter(dsl_cr::user_id.eq(user_id))
            .select(dsl_cr::id)
            .load::<Uuid>(conn)
            .await?;

        Ok(rooms)
    }
}

#[derive(serde::Serialize)]
pub struct CompanyInfo {
    pub id: Uuid,
    pub name: String,
    pub logo: String,
}

#[derive(serde::Serialize)]
pub struct UserInfo {
    pub given_name: String,
    pub family_name: String,
    pub company: Vec<CompanyInfo>,
}

impl UserInfo {
    pub async fn from_id(
        user_id: Uuid,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Option<Self>, Error> {
        if let Some(user_data) = UserData::from_user(User { id: user_id }, conn).await? {
            Ok(Some(UserInfo {
                given_name: user_data.given_name,
                family_name: user_data.family_name,
                company: CompanyUser::company_for_user(user_id, conn).await?,
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(serde::Serialize)]
pub struct Message {
    pub id: i64,
    pub from_user: Uuid,
    pub content: String,
    pub created_at: PrimitiveDateTime,
    pub extra: Option<MessageExtra>,
}

impl Message {
    pub async fn list(
        room_id: Uuid,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Self>, Error> {
        #[derive(Clone, Selectable, Queryable)]
        #[diesel(table_name = crate::schema::chatmessage)]
        #[diesel(check_for_backend(diesel::pg::Pg))]
        struct DbMessage {
            id: i64,
            from_user_id: Uuid,
            content: String,
            created_at: PrimitiveDateTime,
        }

        use crate::schema::chatmessage::dsl as dsl_cm;

        let db_messages = dsl_cm::chatmessage
            .filter(dsl_cm::room_id.eq(room_id))
            .order_by(dsl_cm::id.asc())
            .select(DbMessage::as_select())
            .load::<DbMessage>(conn)
            .await?;
        let mut messages = Vec::with_capacity(db_messages.len());

        use crate::schema::chatcontractoffer::dsl as dsl_cco;
        use crate::schema::chatcontractupdate::dsl as dsl_ccu;

        for db_message in db_messages {
            let mut extra = None;

            let contract_offer = dsl_cco::chatcontractoffer
                .filter(dsl_cco::message_id.eq(db_message.id))
                .select((dsl_cco::id, dsl_cco::offered_payout))
                .first::<(i64, Cents)>(conn)
                .await
                .optional()?;

            if let Some((offer_id, payout)) = contract_offer {
                extra = Some(MessageExtra::ContractCreated {
                    offer_id,
                    payout: payout.0,
                });
            } else {
                let contract_update = dsl_ccu::chatcontractupdate
                    .filter(dsl_ccu::message_id.eq(db_message.id))
                    .select((dsl_ccu::offer_id, dsl_ccu::update_kind))
                    .first::<(i64, ContractStatus)>(conn)
                    .await
                    .optional()?;

                if let Some((offer_id, new_status)) = contract_update {
                    extra = Some(MessageExtra::ContractStatusChange {
                        offer_id,
                        new_status,
                    });
                }
            }

            messages.push(Message {
                id: db_message.id,
                from_user: db_message.from_user_id,
                content: db_message.content,
                created_at: db_message.created_at,
                extra,
            });
        }

        Ok(messages)
    }
}

#[derive(serde::Serialize)]
pub enum MessageExtra {
    ContractCreated {
        offer_id: i64,
        payout: i64,
    },
    ContractStatusChange {
        offer_id: i64,
        new_status: ContractStatus,
    },
}

#[derive(Clone, Selectable, Queryable)]
#[diesel(table_name = crate::schema::chatlastseen)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChatLastSeen {
    pub user_id: Uuid,
    pub last_message_seen_id: i64,
}

impl ChatLastSeen {
    pub async fn list(
        room_id: Uuid,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<Vec<Self>, Error> {
        use crate::schema::chatlastseen::dsl as dsl_cls;

        let last_seens = dsl_cls::chatlastseen
            .filter(dsl_cls::room_id.eq(room_id))
            .select(ChatLastSeen::as_select())
            .load(conn)
            .await?;

        Ok(last_seens)
    }
}
