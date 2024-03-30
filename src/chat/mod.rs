use std::collections::HashMap;

use axum::http::StatusCode;
use diesel::{data_types::Cents, pg::Pg, ExpressionMethods};
use diesel_async::{AsyncConnection, RunQueryDsl};
use time::PrimitiveDateTime;
use uuid::Uuid;

use crate::{models, rpc::RpcServerModule, AppState, Error};

#[derive(serde::Serialize)]
struct ChatRoom {
    users: HashMap<Uuid, models::UserInfo>,
    messages: Vec<models::Message>,
    last_seen_message: HashMap<Uuid, i64>,
}

#[derive(serde::Deserialize)]
struct SubscribeParam {
    room_id: Uuid,
}

async fn subscribe(
    param: SubscribeParam,
    user: models::User,
    state: AppState,
) -> Result<ChatRoom, Error> {
    let mut conn = state.get_conn().await?;

    if let Some(room) = models::ChatRoom::from_id(param.room_id, &mut conn).await? {
        let user_ids = models::CompanyUser::users_in_company(room.company_id, &mut conn)
            .await?
            .into_iter()
            .chain([room.user_id]);

        let mut users = HashMap::default();
        let mut saw_current_user = false;

        for uid in user_ids {
            if user.id == uid {
                saw_current_user = true;
            }

            if let Some(user_info) = models::UserInfo::from_id(uid, &mut conn).await? {
                users.insert(uid, user_info);
            } else {
                tracing::warn!("A user just disappeared!");
            }
        }

        if !saw_current_user {
            return Err(Error::Custom {
                status_code: StatusCode::NOT_FOUND,
                error: "Room of this id was not found".into(),
            });
        }

        Ok(ChatRoom {
            users,
            messages: models::Message::list(room.id, &mut conn).await?,
            last_seen_message: models::ChatLastSeen::list(room.id, &mut conn)
                .await?
                .into_iter()
                .map(|seen| (seen.user_id, seen.last_message_seen_id))
                .collect(),
        })
    } else {
        Err(Error::Custom {
            status_code: StatusCode::NOT_FOUND,
            error: "Room of this id was not found".into(),
        })
    }
}

async fn post(message: Message, user: models::User, state: AppState) -> Result<(), Error> {
    let mut conn = state.get_conn().await?;

    if let Some(room) = models::ChatRoom::from_id(message.room_id, &mut conn).await? {
        let mut users = models::CompanyUser::users_in_company(room.company_id, &mut conn).await?;
        users.push(room.user_id);

        if !users.iter().any(|id| *id == user.id) {
            return Err(Error::Custom {
                status_code: StatusCode::NOT_FOUND,
                error: "Room of this id was not found".into(),
            });
        }

        let message = message.insert(user.id, &mut conn).await?;
        let notification = serde_json::json!({
            "room_id": room.id,
            "message": message,
        });

        for id in users {
            state.send(models::User { id }, notification.clone());
        }

        todo!("Send every user in the room a message on their websockets")
    } else {
        Err(Error::Custom {
            status_code: StatusCode::NOT_FOUND,
            error: "Room of this id was not found".into(),
        })
    }
}

#[derive(Debug, serde::Deserialize)]
struct Message {
    room_id: Uuid,
    message: String,
    extra: Option<MessageExtra>,
}

impl Message {
    async fn insert(
        self,
        user_id: Uuid,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<models::Message, Error> {
        use crate::schema::chatmessage::dsl as dsl_cm;

        let message_data = diesel::insert_into(dsl_cm::chatmessage)
            .values((
                dsl_cm::room_id.eq(self.room_id),
                dsl_cm::from_user_id.eq(user_id),
                dsl_cm::content.eq(&self.message),
            ))
            .returning((dsl_cm::id, dsl_cm::created_at))
            .load::<(i64, PrimitiveDateTime)>(conn)
            .await?;
        let (id, created_at) = message_data[0];

        let extra = if let Some(extra) = self.extra {
            let extra = match extra {
                MessageExtra::ContractCreated { payout } => {
                    use crate::schema::chatcontractoffer::dsl as dsl_cco;

                    let offer_ids = diesel::insert_into(dsl_cco::chatcontractoffer)
                        .values((
                            dsl_cco::message_id.eq(id),
                            dsl_cco::offered_payout.eq(Cents(payout)),
                        ))
                        .returning(dsl_cco::id)
                        .load::<i64>(conn)
                        .await?;

                    models::MessageExtra::ContractCreated {
                        offer_id: offer_ids[0],
                        payout,
                    }
                }
                MessageExtra::ContractStatusChange {
                    offer_id,
                    new_status,
                } => {
                    use crate::schema::chatcontractupdate::dsl as dsl_ccu;

                    diesel::insert_into(dsl_ccu::chatcontractupdate)
                        .values((
                            dsl_ccu::message_id.eq(id),
                            dsl_ccu::offer_id.eq(offer_id),
                            dsl_ccu::update_kind.eq(new_status),
                        ))
                        .execute(conn)
                        .await?;

                    models::MessageExtra::ContractStatusChange {
                        offer_id,
                        new_status,
                    }
                }
            };

            Some(extra)
        } else {
            None
        };

        Ok(models::Message {
            id,
            from_user: user_id,
            content: self.message,
            created_at,
            extra,
        })
    }
}

#[derive(Debug, serde::Deserialize)]
enum MessageExtra {
    ContractCreated {
        payout: i64,
    },
    ContractStatusChange {
        offer_id: i64,
        new_status: models::ContractStatus,
    },
}

pub fn module(modules: RpcServerModule<'_>) {
    modules.add_fn(subscribe).add_fn(post);
}
