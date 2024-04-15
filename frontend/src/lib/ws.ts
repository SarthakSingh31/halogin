interface RpcFunction {
    "chat.create_room": {
        send: {}
        recv: {},
    },
    "chat.list_rooms": {
        send: {}
        recv: {},
    }
}
interface RpcFunctionMessage<K extends keyof RpcFunction> {
    method: K,
    data: RpcFunction[K]['recv'],
    nonce: number,
}
interface RpcFunctionMessageError<K extends keyof RpcFunction> {
    method: K,
    error: string,
    nonce: number,
}
interface RpcEvent {
    "NewMessage": { content: string },
    "NewRoom": { room_id: string },
}
interface RpcEventMessage<K extends keyof RpcEvent> {
    event: K,
    data: RpcEvent[K],
}

type RpcMessage<K extends keyof (RpcEvent | RpcFunction)> = RpcFunctionMessage<K> | RpcFunctionMessageError<K> | RpcEventMessage<K>;

class WsRpc {
    nonce: number;
    ws: WebSocket | null;
    evtHandlers: Map<string, (data: any) => void>;
    callbacks: Map<number, (data: any) => void>;
    pending: Array<(ws: WebSocket) => void>;

    constructor() {
        this.nonce = 0;
        this.ws = null;
        this.evtHandlers = new Map();
        this.callbacks = new Map();
        this.pending = new Array();
    }

    connect() {
        this.ws = new WebSocket("/ws");
        this.ws.onmessage = <K extends keyof (RpcEvent | RpcFunction)>(msg: any) => {
            let message = JSON.parse(msg.data) as RpcMessage<K>;
            if ('event' in message) {
                let handler = this.evtHandlers.get(message.event);
                handler?.call(this, message.data);
            } else if ('nonce' in message) {
                let handler = this.callbacks.get(message.nonce);

                if ('data' in message) {
                    handler?.call(this, message.data);
                } else {
                    console.error(`Error while calling '${message.method}': ${message.error}`);
                }

                this.callbacks.delete(message.nonce);
            }
        };
        this.ws.onopen = (_evt) => {
            this.pending.forEach((pending_fn) => {
                if (this.ws) {
                    pending_fn.call(this, this.ws);
                } else {
                    console.error("WS was null while trying to call the pending functions")
                }
            });
        };
        this.ws.onclose = (_evt) => {
            this.ws = null;

            setTimeout(() => {
                this.connect();
            }, 5000);
        };
    }

    addEventHandler<E extends keyof RpcEvent>(event: E, handler: (data: RpcEvent[E]) => void) {
        this.evtHandlers.set(event, handler);
    }

    call<F extends keyof RpcFunction>(method: F, data: RpcFunction[F]['send'], callback: (resp: RpcFunction[F]['recv']) => void) {
        let nonce = this.nonce;
        this.nonce += 1;

        this.callbacks.set(nonce, callback);

        let msg = JSON.stringify({ method, data, nonce });
        if (this.ws && this.ws.readyState === this.ws.OPEN) {
            this.ws.send(msg);
        } else {
            this.pending.push((ws: WebSocket) => ws.send(msg));
        }
    }
}

export const wsRpc = new WsRpc();