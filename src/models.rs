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

use crate::Error;

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
