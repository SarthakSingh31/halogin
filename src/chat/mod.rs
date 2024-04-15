use std::collections::HashMap;

use axum::{http::StatusCode, Json};
use diesel::{data_types::Cents, pg::Pg, ExpressionMethods};
use diesel_async::{AsyncConnection, RunQueryDsl};
use time::PrimitiveDateTime;
use uuid::Uuid;

use crate::{
    db::User,
    models,
    state::{DbConn, MsgEmitter},
    ws::{WsError, WsFunctions},
    Error,
};

type Result<T> = std::result::Result<T, WsError>;

#[derive(Debug, serde::Deserialize)]
struct NewMessage {
    content: String,
    contract_change: Option<MessageContractChange>,
    attachment: Option<NewMessageFile>,
    change_selected_campaign_to: Option<Uuid>,
}

#[derive(Debug, serde::Deserialize)]
enum MessageContractChange {
    ProposedByCompany {
        campaign_id: Option<Uuid>,
        payout: i64,
    },
    AcceptedByCreator,
    WithdrawnByCompany,
    CancelledByCreator,
    FinishedByCreator,
    ApprovedByCompany,
}

#[derive(Debug, serde::Deserialize)]
struct NewMessageFile {
    name: String,
    // This comes in base64 encoded
    content: Box<[u8]>,
    content_type: String,
}

#[derive(Debug, serde::Deserialize)]
struct CreateChatRoom {
    message: NewMessage,
    direction: CreateChatRoomDirection,
}

#[derive(Debug, serde::Deserialize)]
// Get the user_id from the websocket session
enum CreateChatRoomDirection {
    UserToCompany { company_id: Uuid },
    CompanyToUser { company_id: Uuid, to_user_id: Uuid },
}

#[derive(serde::Serialize)]
struct ChatRoom {
    users: HashMap<Uuid, models::UserInfo>,
    messages: Vec<models::Message>,
    last_seen_message: HashMap<Uuid, i64>,
}

#[derive(serde::Deserialize)]
enum CreateParam {
    WithCompany(Uuid),
    WithUser {
        current_user_company_id: Uuid,
        other_user_id: Uuid,
    },
}

#[derive(Debug, serde::Serialize)]
struct Room {
    room_id: Uuid,
}

async fn list_rooms(
    user: User,
    DbConn { mut conn }: DbConn,
) -> Result<Json<Vec<models::ChatRoom>>> {
    Ok(models::ChatRoom::list(user.id, &mut conn)
        .await
        .map(|rooms| Json(rooms))?)
}

async fn create(
    user: User,
    DbConn { mut conn }: DbConn,
    emitter: MsgEmitter,
    Json(param): Json<CreateParam>,
) -> Result<Json<Uuid>> {
    let (company_id, user_id) = match param {
        CreateParam::WithCompany(company_id) => (company_id, user.id),
        CreateParam::WithUser {
            current_user_company_id,
            other_user_id,
        } => {
            let current_user_info = models::UserInfo::from_id(user.id, &mut conn)
                .await?
                .expect("Failed to find the user who is the owner of the current session");
            if current_user_info
                .company
                .into_iter()
                .any(|company_id| company_id.id == current_user_company_id)
            {
                (current_user_company_id, other_user_id)
            } else {
                return Err(WsError::Custom {
                    reason: "You are not in that company".into(),
                });
            }
        }
    };

    let users_in_company = models::CompanyUser::users_in_company(company_id, &mut conn).await?;
    if users_in_company.iter().any(|id| *id == user_id) {
        return Err(WsError::Custom {
            reason: "Cannot make a chat with a user of the same company".into(),
        });
    }

    let room_id = models::ChatRoom::create(company_id, user_id, &mut conn).await?;
    let user_ids = users_in_company.into_iter().chain([user_id]);

    for id in user_ids {
        emitter
            .send(
                id,
                Some(serde_json::json!({
                    "kind": "chat.new_room",
                    "data": {
                        "room_id": room_id,
                    },
                })),
                None,
                &mut conn,
            )
            .await?;
    }

    Ok(Json(room_id))
}

#[derive(serde::Deserialize)]
struct SubscribeParam {
    room_id: Uuid,
}

// async fn subscribe(
//     user: models::User,
//     Json(param): Json<SubscribeParam>,
//     DbConn { mut conn }: DbConn,
// ) -> Result<ChatRoom> {
//     if let Some(room) = models::ChatRoom::from_id(param.room_id, &mut conn).await? {
//         let user_ids = models::CompanyUser::users_in_company(room.company_id, &mut conn)
//             .await?
//             .into_iter()
//             .chain([room.user_id]);

//         let mut users = HashMap::default();
//         let mut saw_current_user = false;

//         for uid in user_ids {
//             if user.id == uid {
//                 saw_current_user = true;
//             }

//             if let Some(user_info) = models::UserInfo::from_id(uid, &mut conn).await? {
//                 users.insert(uid, user_info);
//             } else {
//                 tracing::warn!("A user just disappeared!");
//             }
//         }

//         if !saw_current_user {
//             return Err(Error::Custom {
//                 status_code: StatusCode::NOT_FOUND,
//                 error: "Room of this id was not found".into(),
//             });
//         }

//         Ok(ChatRoom {
//             users,
//             messages: models::Message::list(room.id, &mut conn).await?,
//             last_seen_message: models::ChatLastSeen::list(room.id, &mut conn)
//                 .await?
//                 .into_iter()
//                 .map(|seen| (seen.user_id, seen.last_message_seen_id))
//                 .collect(),
//         })
//     } else {
//         Err(Error::Custom {
//             status_code: StatusCode::NOT_FOUND,
//             error: "Room of this id was not found".into(),
//         })
//     }
// }

// async fn post(
//     user: models::User,
//     Json(message): Json<Message>,
//     DbConn { mut conn }: DbConn,
//     emitter: MsgEmitter,
// ) -> Result<()> {
//     if let Some(room) = models::ChatRoom::from_id(message.room_id, &mut conn).await? {
//         let mut users = models::CompanyUser::users_in_company(room.company_id, &mut conn).await?;
//         users.push(room.user_id);

//         if !users.iter().any(|id| *id == user.id) {
//             return Err(Error::Custom {
//                 status_code: StatusCode::NOT_FOUND,
//                 error: "Room of this id was not found".into(),
//             });
//         }

//         let message = message.insert(user.id, &mut conn).await?;

//         for id in users {
//             emitter
//                 .send(
//                     id,
//                     Some(serde_json::json!({
//                         "kind": "chat.message",
//                         "data": {
//                             "room_id": room.id,
//                             "message": message,
//                         },
//                     })),
//                     None,
//                     &mut conn,
//                 )
//                 .await?;
//         }

//         todo!("Send every user in the room a message on their websockets")
//     } else {
//         Err(Error::Custom {
//             status_code: StatusCode::NOT_FOUND,
//             error: "Room of this id was not found".into(),
//         })
//     }
// }

// #[derive(Debug, serde::Deserialize)]
// struct Message {
//     room_id: Uuid,
//     message: String,
//     extra: Option<MessageExtra>,
// }

// impl Message {
//     async fn insert(
//         self,
//         user_id: Uuid,
//         conn: &mut impl AsyncConnection<Backend = Pg>,
//     ) -> Result<models::Message> {
//         use crate::schema::chatmessage::dsl as dsl_cm;

//         let message_data = diesel::insert_into(dsl_cm::chatmessage)
//             .values((
//                 dsl_cm::room_id.eq(self.room_id),
//                 dsl_cm::from_user_id.eq(user_id),
//                 dsl_cm::content.eq(&self.message),
//             ))
//             .returning((dsl_cm::id, dsl_cm::created_at))
//             .load::<(i64, PrimitiveDateTime)>(conn)
//             .await?;
//         let (id, created_at) = message_data[0];

//         let extra = if let Some(extra) = self.extra {
//             let extra = match extra {
//                 MessageExtra::ContractOfferCreated { payout } => {
//                     use crate::schema::chatcontractoffer::dsl as dsl_cco;

//                     let offer_ids = diesel::insert_into(dsl_cco::chatcontractoffer)
//                         .values((
//                             dsl_cco::message_id.eq(id),
//                             dsl_cco::offered_payout.eq(Cents(payout)),
//                         ))
//                         .returning(dsl_cco::id)
//                         .load::<i64>(conn)
//                         .await?;

//                     models::MessageExtra::ContractOfferCreated {
//                         offer_id: offer_ids[0],
//                         payout,
//                     }
//                 }
//                 MessageExtra::ContractOfferStatusChange {
//                     offer_id,
//                     new_status,
//                 } => {
//                     use crate::schema::chatcontractofferupdate::dsl as dsl_ccou;

//                     diesel::insert_into(dsl_ccou::chatcontractofferupdate)
//                         .values((
//                             dsl_ccou::message_id.eq(id),
//                             dsl_ccou::offer_id.eq(offer_id),
//                             dsl_ccou::update_kind.eq(new_status),
//                         ))
//                         .execute(conn)
//                         .await?;

//                     models::MessageExtra::ContractOfferStatusChange {
//                         offer_id,
//                         new_status,
//                     }
//                 }
//             };

//             Some(extra)
//         } else {
//             None
//         };

//         Ok(models::Message {
//             id,
//             from_user: user_id,
//             content: self.message,
//             created_at,
//             extra,
//         })
//     }
// }

#[derive(Debug, serde::Deserialize)]
enum MessageExtra {
    ContractOfferCreated {
        payout: i64,
    },
    ContractOfferStatusChange {
        offer_id: i64,
        new_status: models::ContractOfferStatus,
    },
}

pub fn functions() -> WsFunctions {
    WsFunctions::default().add(list_rooms).add(create)
}
