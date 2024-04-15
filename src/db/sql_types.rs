use std::io::Write;

use diesel::{
    deserialize::{FromSql, FromSqlRow, Result as DerResult},
    expression::AsExpression,
    pg::{Pg, PgValue},
    serialize::{IsNull, Output, Result as SerResult, ToSql},
};

#[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
#[diesel(postgres_type(name = "contractofferstatus"))]
pub struct Contractofferstatus;

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
#[diesel(sql_type = Contractofferstatus)]
pub enum ContractOfferStatus {
    AcceptedByCreator,
    WithdrawnByCompany,
    CancelledByCreator,
    FinishedByCreator,
    ApprovedByCompany,
}

impl ToSql<Contractofferstatus, Pg> for ContractOfferStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> SerResult {
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

impl FromSql<Contractofferstatus, Pg> for ContractOfferStatus {
    fn from_sql(bytes: PgValue<'_>) -> DerResult<Self> {
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
