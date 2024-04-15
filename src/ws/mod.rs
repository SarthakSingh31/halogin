use std::pin::Pin;

use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::Response,
    Json,
};
use futures::{Future, SinkExt, StreamExt};
use fxhash::FxHashMap;
use tokio::sync::mpsc;

use crate::{
    db::User,
    state::{AppState, Session, SessionWithPage},
};

#[derive(Default)]
pub struct WsFunctions(FxHashMap<String, Box<dyn WsFuncErased>>);

impl WsFunctions {
    pub fn add_scoped(mut self, scope: &str, fns: WsFunctions) -> Self {
        for (name, func) in fns.0 {
            self.0.insert(format!("{scope}.{name}"), func);
        }

        self
    }

    pub fn add<T: 'static, F: WsFunc<T>>(mut self, func: F) -> Self {
        self.0.insert(func.name().into(), func.boxed().erased());
        self
    }

    pub async fn call(
        &self,
        name: &str,
        data: serde_json::Value,
        session: &SessionWithPage,
        user: User,
        state: &AppState,
    ) -> Result<serde_json::Value, WsError> {
        self.0
            .get(name)
            .ok_or(WsError::FunctionNotFound { name: name.into() })?
            .call_erased(data, session, user, state)
            .await
    }
}

pub trait WsFunc<T: 'static>: Send + Sync + 'static {
    fn call<'c>(
        &'c self,
        data: serde_json::Value,
        session: &'c SessionWithPage,
        user: User,
        state: &'c AppState,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, WsError>> + Send + 'c>>;

    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
            .split("::")
            .last()
            .expect("Function has no name")
    }

    fn boxed(self) -> WsFuncBoxed<T>
    where
        Self: Sized,
    {
        WsFuncBoxed(Box::new(self))
    }
}

pub struct WsFuncBoxed<T>(Box<dyn WsFunc<T>>);

impl<T: 'static> WsFuncBoxed<T> {
    pub fn erased(self) -> Box<dyn WsFuncErased> {
        Box::new(self)
    }
}

pub trait WsFuncErased: Send + Sync + 'static {
    fn call_erased<'c>(
        &'c self,
        data: serde_json::Value,
        session: &'c SessionWithPage,
        user: User,
        state: &'c AppState,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, WsError>> + Send + 'c>>;
}

impl<T: 'static> WsFuncErased for WsFuncBoxed<T> {
    fn call_erased<'c>(
        &'c self,
        data: serde_json::Value,
        session: &'c SessionWithPage,
        user: User,
        state: &'c AppState,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, WsError>> + Send + 'c>> {
        self.0.call(data, session, user, state)
    }
}

pub trait WsFuncParam: Sized + Send + 'static {
    fn make<'m>(
        data: &'m serde_json::Value,
        session: &'m SessionWithPage,
        user: User,
        state: &'m AppState,
    ) -> impl Future<Output = Result<Self, WsError>> + Send + 'm;
}

#[derive(Debug, thiserror::Error)]
pub enum WsError {
    #[error("Invalid call beacuse: {reason}")]
    Custom { reason: String },
    #[error("An inner error occured: {0:?}")]
    InnerError(#[from] crate::Error),
    #[error("There is no function: {name}")]
    FunctionNotFound { name: String },
    #[error("Failed to get connection from pool: {0:?}")]
    PoolError(#[from] diesel_async::pooled_connection::deadpool::PoolError),
    #[error("Failed to parse json: {0:?}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("A error from Axum: {0:?}")]
    AxumError(#[from] axum::Error),
}

impl serde::Serialize for WsError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{self:?}"))
    }
}

macro_rules! impl_ws_func_inner {
    ($($t:ident),*) => {
        impl<F, Fut, R, $($t),*> WsFunc<($($t),*,)> for F
        where
            Fut: Future<Output = Result<Json<R>, WsError>> + Send,
            F: Fn($($t),*) -> Fut + Send + Sync + 'static,
            R: serde::Serialize,
            $($t: WsFuncParam),*
        {
            fn call<'c>(
                &'c self,
                data: serde_json::Value,
                session: &'c SessionWithPage,
                user: User,
                state: &'c AppState,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, WsError>> + Send + 'c>> {
                Box::pin(async move {
                    let resp = self($($t::make(&data, session, user, state).await?),*).await?;
                    Ok(serde_json::value::to_value(resp.0)?)
                })
            }
        }
    };
}

impl_ws_func_inner!(T1);
impl_ws_func_inner!(T1, T2);
impl_ws_func_inner!(T1, T2, T3);
impl_ws_func_inner!(T1, T2, T3, T4);
impl_ws_func_inner!(T1, T2, T3, T4, T5);
impl_ws_func_inner!(T1, T2, T3, T4, T5, T6);
impl_ws_func_inner!(T1, T2, T3, T4, T5, T6, T7);
impl_ws_func_inner!(T1, T2, T3, T4, T5, T6, T7, T8);

impl<T: serde::de::DeserializeOwned + Send + 'static> WsFuncParam for Json<T> {
    async fn make<'m>(
        data: &'m serde_json::Value,
        _session: &'m SessionWithPage,
        _user: User,
        _state: &'m AppState,
    ) -> Result<Self, WsError> {
        Ok(Json(serde_json::value::from_value(data.clone())?))
    }
}

impl WsFuncParam for User {
    async fn make<'m>(
        _data: &'m serde_json::Value,
        _session: &'m SessionWithPage,
        user: User,
        _state: &'m AppState,
    ) -> Result<Self, WsError> {
        Ok(user)
    }
}

pub async fn connect(
    ws: WebSocketUpgrade,
    session: Session,
    user: User,
    State(state): State<AppState>,
) -> Response {
    println!("here");
    ws.on_upgrade(move |ws| handle_socket(ws, session, user, state))
}

#[derive(serde::Deserialize)]
struct FuncCallMessage {
    method: String,
    data: serde_json::Value,
    nonce: usize,
}

#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum WsResponse {
    MethodCallSuccess {
        method: String,
        data: serde_json::Value,
        nonce: usize,
    },
    MethodCallError {
        method: String,
        error: WsError,
        nonce: usize,
    },
    RawError {
        error: WsError,
    },
    Event {
        event: String,
        data: serde_json::Value,
    },
}

async fn handle_socket(ws: WebSocket, session: Session, user: User, state: AppState) {
    let funcs = state.ws_funcs();

    let (mut ws_tx, mut ws_rx) = ws.split();
    let (proxy_tx, mut proxy_rx) = mpsc::unbounded_channel::<WsResponse>();

    tokio::spawn(async move {
        while let Some(msg) = proxy_rx.recv().await {
            match serde_json::to_string(&msg) {
                Ok(msg) => {
                    if let Err(err) = ws_tx.send(axum::extract::ws::Message::Text(msg)).await {
                        tracing::error!("Failed to respond due to error: {err:?}");
                    }
                }
                Err(err) => {
                    tracing::error!("Failed to respond with message: {msg:?} due to err: {err:?}");
                }
            }
        }
    });

    let ws_tx = proxy_tx.clone();
    let page = session.add_page(ws_tx).await;

    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(msg) => match msg {
                axum::extract::ws::Message::Text(msg) => {
                    let call: FuncCallMessage = match serde_json::from_str(&msg) {
                        Ok(call) => call,
                        Err(err) => {
                            if proxy_tx
                                .send(WsResponse::RawError { error: err.into() })
                                .is_err()
                            {
                                tracing::error!("Failed to send a message over ws");
                            }
                            continue;
                        }
                    };

                    let resp = funcs
                        .call(&call.method, call.data, &page, user, &state)
                        .await
                        .map(|response| WsResponse::MethodCallSuccess {
                            method: call.method.clone(),
                            data: response,
                            nonce: call.nonce,
                        })
                        .unwrap_or_else(|err| WsResponse::MethodCallError {
                            method: call.method,
                            error: err,
                            nonce: call.nonce,
                        });
                    if proxy_tx.send(resp).is_err() {
                        tracing::error!("Failed to send a message over ws");
                    }
                }
                axum::extract::ws::Message::Close(_) => page.close().await,
                _ => continue,
            },
            Err(err) => {
                if proxy_tx
                    .send(WsResponse::RawError { error: err.into() })
                    .is_err()
                {
                    tracing::error!("Failed to send a message over ws");
                }
            }
        }
    }

    // Do a second close here just in case there was no close message
    page.close().await
}
