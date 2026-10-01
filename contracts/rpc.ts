/** Daemon RPC methods: JSON-RPC 2.0, newline-delimited, over a Unix socket. Source of truth for every client. */
export type RpcMethods = {
  "daemon.ping": { result: { pong: true } };
};

export type RpcMethodName = keyof RpcMethods;
