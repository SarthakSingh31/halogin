use std::io::Write;

use diesel::{
    data_types::Cents,
    deserialize::{self, FromSql, FromSqlRow},
    pg::{Pg, PgValue},
    prelude::*,
    serialize::{self, IsNull, Output, ToSql},
    AsExpression,
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use time::PrimitiveDateTime;
use uuid::Uuid;

use crate::{db::User, Error};

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

#[derive(Insertable, Queryable)]
#[diesel(table_name = crate::schema::company)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Company<'c> {
    pub id: Uuid,
    pub full_name: &'c str,
    pub banner_desc: &'c str,
    pub logo_url: &'c str,
    pub industry: &'c [Option<&'c str>],
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
#[diesel(sql_type = crate::schema::sql_types::Contractofferstatus)]
pub enum ContractOfferStatus {
    AcceptedByCreator,
    WithdrawnByCompany,
    CancelledByCreator,
    FinishedByCreator,
    ApprovedByCompany,
}

impl ToSql<crate::schema::sql_types::Contractofferstatus, Pg> for ContractOfferStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            ContractOfferStatus::AcceptedByCreator => out.write_all(b"AcceptedByCreator")?,
            ContractOfferStatus::WithdrawnByCompany => out.write_all(b"WithdrawnByCompany")?,
            ContractOfferStatus::CancelledByCreator => out.write_all(b"CancelledByCreator")?,
            ContractOfferStatus::FinishedByCreator => out.write_all(b"FinishedByCreator")?,
            ContractOfferStatus::ApprovedByCompany => out.write_all(b"ApprovedByCompany")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<crate::schema::sql_types::Contractofferstatus, Pg> for ContractOfferStatus {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"AcceptedByCreator" => Ok(ContractOfferStatus::AcceptedByCreator),
            b"WithdrawnByCompany" => Ok(ContractOfferStatus::WithdrawnByCompany),
            b"CancelledByCreator" => Ok(ContractOfferStatus::CancelledByCreator),
            b"FinishedByCreator" => Ok(ContractOfferStatus::FinishedByCreator),
            b"ApprovedByCompany" => Ok(ContractOfferStatus::ApprovedByCompany),
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
            let (name, logo_url) = dsl_c::company
                .filter(dsl_c::id.eq(company_id))
                .select((dsl_c::full_name, dsl_c::logo_url))
                .first::<(String, String)>(conn)
                .await?;
            companies.push(CompanyInfo {
                id: company_id,
                name,
                logo_url,
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
#[diesel(table_name = crate::schema::sessionfcmtoken)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SessionFcmToken {
    pub token: String,
    pub session_token: String,
}

impl SessionFcmToken {
    pub async fn delete(
        token: &str,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<(), Error> {
        use crate::schema::sessionfcmtoken::dsl as dsl_uft;

        diesel::delete(dsl_uft::sessionfcmtoken)
            .filter(dsl_uft::token.eq(token))
            .execute(conn)
            .await?;

        Ok(())
    }
}

#[derive(Clone, Insertable, Queryable, AsChangeset, Selectable, serde::Serialize)]
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
    ) -> Result<Vec<Self>, Error> {
        use crate::schema::chatroom::dsl as dsl_cr;

        let rooms = dsl_cr::chatroom
            .filter(dsl_cr::user_id.eq(user_id))
            .select(Self::as_select())
            .load::<Self>(conn)
            .await?;

        Ok(rooms)
    }
}

#[derive(serde::Serialize)]
pub struct CompanyInfo {
    pub id: Uuid,
    pub name: String,
    pub logo_url: String,
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
        use crate::schema::chatcontractofferupdate::dsl as dsl_ccou;

        for db_message in db_messages {
            let mut extra = None;

            let contract_offer = dsl_cco::chatcontractoffer
                .filter(dsl_cco::message_id.eq(db_message.id))
                .select((dsl_cco::id, dsl_cco::offered_payout))
                .first::<(i64, Cents)>(conn)
                .await
                .optional()?;

            if let Some((offer_id, payout)) = contract_offer {
                extra = Some(MessageExtra::ContractOfferCreated {
                    offer_id,
                    payout: payout.0,
                });
            } else {
                let contract_update = dsl_ccou::chatcontractofferupdate
                    .filter(dsl_ccou::message_id.eq(db_message.id))
                    .select((dsl_ccou::offer_id, dsl_ccou::update_kind))
                    .first::<(i64, ContractOfferStatus)>(conn)
                    .await
                    .optional()?;

                if let Some((offer_id, new_status)) = contract_update {
                    extra = Some(MessageExtra::ContractOfferStatusChange {
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
    ContractOfferCreated {
        offer_id: i64,
        payout: i64,
    },
    ContractOfferStatusChange {
        offer_id: i64,
        new_status: ContractOfferStatus,
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
