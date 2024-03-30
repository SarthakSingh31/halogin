use std::{pin::Pin, sync::Arc};

use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::Response,
    routing, Router,
};
use futures::{Future, SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::{models::User, AppState, Error};

pub trait RpcFn<I, O, Fut>: Send + Sync
where
    I: for<'de> serde::Deserialize<'de>,
    O: serde::Serialize + 'static,
    Fut: Future<Output = Result<O, Error>> + Send,
{
    fn call(&self, data: I, user: User, state: AppState) -> Fut;

    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
            .split("::")
            .last()
            .expect("The function has no name?")
    }
}

struct RpcFnObj<I, O, Fut>(Box<dyn RpcFn<I, O, Fut>>)
where
    I: for<'de> serde::Deserialize<'de>,
    O: serde::Serialize + 'static,
    Fut: Future<Output = Result<O, Error>> + Send;

trait RpcFnErased: Send + Sync + 'static {
    fn call<'s>(
        &'s self,
        data: serde_json::Value,
        user: User,
        state: AppState,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, Error>> + Send + 's>>;
}

impl<I, O, Fut, Func> RpcFn<I, O, Fut> for Func
where
    I: for<'de> serde::Deserialize<'de>,
    O: serde::Serialize + 'static,
    Fut: Future<Output = Result<O, Error>> + Send,
    Func: Fn(I, User, AppState) -> Fut + Send + Sync + 'static,
{
    fn call(&self, data: I, user: User, state: AppState) -> Fut {
        self(data, user, state)
    }
}

impl<I, O, Fut> RpcFn<I, O, Fut> for RpcFnObj<I, O, Fut>
where
    I: for<'de> serde::Deserialize<'de>,
    O: serde::Serialize + 'static,
    Fut: Future<Output = Result<O, Error>> + Send,
{
    fn call(&self, data: I, user: User, state: AppState) -> Fut {
        self.0.call(data, user, state)
    }

    fn name(&self) -> &'static str {
        self.0.name()
    }
}

impl<I, O, Fut> RpcFnErased for RpcFnObj<I, O, Fut>
where
    I: for<'de> serde::Deserialize<'de> + 'static,
    O: serde::Serialize + 'static,
    Fut: Future<Output = Result<O, Error>> + Send + 'static,
{
    fn call<'s>(
        &'s self,
        data: serde_json::Value,
        user: User,
        state: AppState,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, Error>> + Send + 's>> {
        Box::pin(async move {
            let input: I = serde_json::value::from_value(data)?;
            let output = RpcFn::call(self, input, user, state).await?;

            Ok(serde_json::value::to_value(output)?)
        })
    }
}

#[derive(Default)]
pub struct RpcServer {
    fns: fxhash::FxHashMap<&'static str, fxhash::FxHashMap<&'static str, Box<dyn RpcFnErased>>>,
}
impl RpcServer {
    pub fn add_module(
        mut self,
        namespace: &'static str,
        adder: impl Fn(RpcServerModule<'_>),
    ) -> Self {
        adder(RpcServerModule {
            namespace,
            fns: &mut self.fns,
        });

        self
    }

    async fn call(
        &self,
        namespace: &str,
        method: &str,
        data: serde_json::Value,
        user: User,
        state: AppState,
    ) -> Result<serde_json::Value, Error> {
        self.fns
            .get(namespace)
            .ok_or(Error::RpcMissingNamespace)?
            .get(method)
            .ok_or(Error::RpcMissingMethod)?
            .call(data, user, state)
            .await
    }
}

pub struct RpcServerModule<'f> {
    namespace: &'static str,
    fns: &'f mut fxhash::FxHashMap<
        &'static str,
        fxhash::FxHashMap<&'static str, Box<dyn RpcFnErased>>,
    >,
}

impl<'f> RpcServerModule<'f> {
    pub fn add_fn<I, O, Fut, Func>(self, func: Func) -> Self
    where
        I: for<'de> serde::Deserialize<'de> + 'static,
        O: serde::Serialize + 'static,
        Fut: Future<Output = Result<O, Error>> + Send + 'static,
        Func: Fn(I, User, AppState) -> Fut + Send + Sync + 'static,
    {
        self.fns
            .entry(self.namespace)
            .or_default()
            .insert(func.name(), Box::new(RpcFnObj(Box::new(func))));

        self
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct RpcCall {
    func: String,
    #[serde(default)]
    data: serde_json::Value,
    nonce: usize,
}

pub fn router(rpc_server: RpcServer) -> Router<AppState> {
    let rpc_server = Arc::new(rpc_server);

    Router::new().route(
        "/connect",
        routing::get(
            move |ws, user, state| async move { connect(ws, user, state, rpc_server).await },
        ),
    )
}

async fn connect(
    ws: WebSocketUpgrade,
    user: User,
    State(state): State<AppState>,
    rpc_server: Arc<RpcServer>,
) -> Response {
    ws.on_upgrade(move |ws| handle_socket(ws, user, state, rpc_server))
}

async fn handle_socket(ws: WebSocket, user: User, state: AppState, rpc_server: Arc<RpcServer>) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (mut ws_tx, mut ws_rx) = ws.split();

    let key = state.insert_user_tx(user, tx.clone());

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(err) = ws_tx
                .send(axum::extract::ws::Message::Text(
                    serde_json::to_string(&msg).expect("Failed to serialize the message"),
                ))
                .await
            {
                tracing::error!("Failed to send the message over RPC WS: {err:?}");
            }
        }
    });

    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(msg) => match msg {
                axum::extract::ws::Message::Text(msg) => match serde_json::from_str::<RpcCall>(&msg)
                {
                    Ok(rpc) => if let Some((namespace, method)) = rpc.func.split_once('.') {
                        match rpc_server.call(namespace, method, rpc.data, user, state.clone()).await {
                                    Ok(resp) => if !resp.is_null() && tx.send(serde_json::json!({
                                        "nonce": rpc.nonce,
                                        "response": resp,
                                    })).is_err() {
                                        tracing::error!("Failed to reply to RPC WS with an response");
                                    },
                                    Err(err) => if tx.send(serde_json::json!({
                                            "nonce": rpc.nonce,
                                            "error": format!("Error while trying to call ({}): {err}", rpc.func),
                                    })).is_err() {
                                        tracing::error!("Failed to reply to RPC WS with an error");
                                    },
                                }
                    } else if tx
                        .send(serde_json::json!({
                                "nonce": rpc.nonce,
                                "error": format!("RPC func not formatted properly: {}", rpc.func),
                        }))
                        .is_err()
                    {
                        tracing::error!("Failed to reply to RPC WS with an error");
                    },
                    Err(err) => {
                        if tx
                            .send(serde_json::json!({
                                "error": format!("Failed to parse the sent message: {err:?}"),
                            }))
                            .is_err()
                        {
                            tracing::error!("Failed to reply to RPC WS with an error");
                        }
                    }
                },
                _ => {
                    if tx
                        .send(serde_json::json!({ "error": "Recived value is not text"}))
                        .is_err()
                    {
                        tracing::error!("Failed to reply to RPC WS with an error");
                    }
                }
            },
            Err(err) => {
                if tx
                    .send(serde_json::json!({
                        "error": format!("Failed to read last value sent over the WS: {err}"),
                    }))
                    .is_err()
                {
                    tracing::error!("Failed to reply to RPC WS with an error");
                }
            }
        }
    }

    state.remove(user, key);
}
