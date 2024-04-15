use std::{path::Path, sync::Arc};

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::COOKIE, request::Parts},
};
use dashmap::DashMap;
use diesel::{pg::Pg, ExpressionMethods, JoinOnDsl, QueryDsl};
use diesel_async::{
    pooled_connection::deadpool::{Object, Pool},
    AsyncConnection, AsyncPgConnection, RunQueryDsl,
};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::{
    db::{Encoder, User},
    ws::{WsError, WsFuncParam, WsFunctions, WsResponse},
    Error, SESSION_COOKIE_NAME,
};

#[derive(Clone, Copy)]
pub struct AppState {
    pub(super) pool: &'static Pool<AsyncPgConnection>,
    sessions: &'static DashMap<String, Arc<RwLock<SessionState>>>,
    ws_funcs: &'static WsFunctions,
    fcm_tx: &'static mpsc::UnboundedSender<fcm::Message>,
    config: Config,
    encoder: Encoder,
}

impl AppState {
    pub async fn new(
        db_url: &str,
        fcm_tx: mpsc::UnboundedSender<fcm::Message>,
        ws_funcs: WsFunctions,
        config: Config,
    ) -> Self {
        Self {
            pool: {
                let config =
                    diesel_async::pooled_connection::AsyncDieselConnectionManager::new(db_url);

                let pool = Pool::<AsyncPgConnection>::builder(config)
                    .build()
                    .expect("Failed to build the pool");

                Box::leak(Box::new(pool))
            },
            sessions: Box::leak(Box::default()),
            ws_funcs: Box::leak(Box::new(ws_funcs)),
            fcm_tx: Box::leak(Box::new(fcm_tx)),
            config,
            encoder: Encoder::new().await,
        }
    }

    pub async fn get_conn(&self) -> Result<impl AsyncConnection<Backend = Pg>, Error> {
        self.pool.get().await.map_err(|err| err.into())
    }

    pub fn ws_funcs(&self) -> &'static WsFunctions {
        self.ws_funcs
    }

    pub fn config(&self) -> Config {
        self.config
    }
}

pub struct DbConn {
    pub conn: Object<AsyncPgConnection>,
}

#[async_trait]
impl FromRequestParts<AppState> for DbConn {
    type Rejection = Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let conn = state.pool.get().await?;
        Ok(DbConn { conn })
    }
}

impl WsFuncParam for DbConn {
    async fn make<'m>(
        _data: &'m serde_json::Value,
        _session: &'m SessionWithPage,
        _user: User,
        state: &'m AppState,
    ) -> Result<Self, WsError> {
        let conn = state.pool.get().await?;
        Ok(DbConn { conn })
    }
}

pub struct MsgEmitter {
    fcm_tx: &'static mpsc::UnboundedSender<fcm::Message>,
}

impl MsgEmitter {
    pub async fn send(
        &self,
        user_id: Uuid,
        msg_data: Option<serde_json::Value>,
        msg_notif: Option<fcm::Notification>,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<(), Error> {
        use crate::schema::innerusersession::dsl as dsl_ius;
        use crate::schema::sessionfcmtoken::dsl as dsl_sft;

        let room_user_tokens = dsl_ius::innerusersession
            .filter(dsl_ius::user_id.eq(user_id))
            .inner_join(dsl_sft::sessionfcmtoken.on(dsl_sft::session_token.eq(dsl_ius::token)))
            .select(dsl_sft::token)
            .distinct()
            .load::<String>(conn)
            .await?;

        for token in room_user_tokens {
            if self
                .fcm_tx
                .send(fcm::Message {
                    data: msg_data.clone(),
                    notification: msg_notif.clone(),
                    target: fcm::Target::Token(token),
                    android: None,
                    webpush: None,
                    apns: None,
                    fcm_options: None,
                })
                .is_err()
            {
                tracing::error!("Failed to send fcm message to the fcm client thread");
            }
        }

        Ok(())
    }
}

#[async_trait]
impl FromRequestParts<AppState> for MsgEmitter {
    type Rejection = Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(MsgEmitter {
            fcm_tx: state.fcm_tx,
        })
    }
}

impl WsFuncParam for MsgEmitter {
    async fn make<'m>(
        _data: &'m serde_json::Value,
        _session: &'m SessionWithPage,
        _user: User,
        state: &'m AppState,
    ) -> Result<Self, WsError> {
        Ok(MsgEmitter {
            fcm_tx: state.fcm_tx,
        })
    }
}

pub struct OpenPageState {
    ws_tx: mpsc::UnboundedSender<WsResponse>,
    currently_viewing: bool,
}

slotmap::new_key_type! { struct PageKey;  }

#[derive(Default)]
pub struct SessionState {
    pages: slotmap::DenseSlotMap<PageKey, OpenPageState>,
}

#[derive(Clone)]
pub struct Session {
    session_token: String,
    state: Arc<RwLock<SessionState>>,
    fcm_tx: &'static mpsc::UnboundedSender<fcm::Message>,
}

impl Session {
    pub async fn add_page(&self, ws_tx: mpsc::UnboundedSender<WsResponse>) -> SessionWithPage {
        let mut state = self.state.write().await;
        let page_key = state.pages.insert(OpenPageState {
            ws_tx,
            currently_viewing: false,
        });

        SessionWithPage {
            session: self.clone(),
            page_key,
        }
    }

    pub async fn notify(
        &self,
        data: Option<serde_json::Value>,
        notification: Option<fcm::Notification>,
        webpush: Option<fcm::WebpushConfig>,
        conn: &mut impl AsyncConnection<Backend = Pg>,
    ) -> Result<(), Error> {
        let state = self.state.read().await;
        if state.pages.is_empty() {
            use crate::schema::sessionfcmtoken::dsl as dsl_sft;

            let fcm_token = dsl_sft::sessionfcmtoken
                .filter(dsl_sft::session_token.eq(&self.session_token))
                .select(dsl_sft::token)
                .first::<String>(conn)
                .await?;

            if self
                .fcm_tx
                .send(fcm::Message {
                    data,
                    notification,
                    target: fcm::Target::Token(fcm_token),
                    android: None,
                    webpush,
                    apns: None,
                    fcm_options: None,
                })
                .is_err()
            {
                tracing::error!("Failed to send fcm message to the fcm client thread");
            }
        } else {
            for (_, page) in &state.pages {
                let msg = WsResponse::Event {
                    event: "NewMessage".into(),
                    data: serde_json::json!({
                        "data": data,
                        "notification": notification,
                    }),
                };

                if page.ws_tx.send(msg).is_err() {
                    tracing::error!("Failed to notify and send message to page");
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl FromRequestParts<AppState> for Session {
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
                            return Ok(Session {
                                session_token: value.into(),
                                state: state.sessions.entry(value.into()).or_default().clone(),
                                fcm_tx: state.fcm_tx,
                            });
                        }
                    }
                }
            }
        }

        Err(Error::Unauthorized)
    }
}

impl WsFuncParam for SessionWithPage {
    async fn make<'m>(
        _data: &'m serde_json::Value,
        session: &'m SessionWithPage,
        _user: User,
        _state: &'m AppState,
    ) -> Result<Self, WsError> {
        Ok(session.clone())
    }
}

#[derive(Clone)]
pub struct SessionWithPage {
    session: Session,
    page_key: PageKey,
}

impl SessionWithPage {
    pub async fn close(&self) {
        self.session.state.write().await.pages.remove(self.page_key);
    }
}

pub struct AllSessions(pub &'static DashMap<String, Arc<RwLock<SessionState>>>);

#[async_trait]
impl FromRequestParts<AppState> for AllSessions {
    type Rejection = Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(AllSessions(state.sessions))
    }
}

impl WsFuncParam for AllSessions {
    async fn make<'m>(
        _data: &'m serde_json::Value,
        _session: &'m SessionWithPage,
        _user: User,
        state: &'m AppState,
    ) -> Result<Self, WsError> {
        Ok(AllSessions(state.sessions))
    }
}

#[async_trait]
impl<'f> FromRequestParts<AppState> for &'f WsFunctions {
    type Rejection = crate::Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(state.ws_funcs)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub storage_path: &'static Path,
}

#[async_trait]
impl FromRequestParts<AppState> for Config {
    type Rejection = crate::Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(state.config)
    }
}
#[async_trait]
impl FromRequestParts<AppState> for Encoder {
    type Rejection = Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(state.encoder)
    }
}

impl WsFuncParam for Encoder {
    async fn make<'m>(
        _data: &'m serde_json::Value,
        _session: &'m SessionWithPage,
        _user: User,
        state: &'m AppState,
    ) -> Result<Self, WsError> {
        Ok(state.encoder)
    }
}
