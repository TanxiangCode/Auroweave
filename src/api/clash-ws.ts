/**
 * WebSocket 客户端封装
 * 作者: TanXiang
 *
 * 功能：
 * - 自动重连（指数退避）
 * - 连接状态管理
 * - 消息类型化分发
 */
import {
  WS_PATH_TRAFFIC,
  WS_PATH_CONNECTIONS,
  WS_PATH_LOGS,
  WS_RECONNECT_DELAY_MS,
  WS_RECONNECT_MAX_DELAY_MS,
  WS_RECONNECT_MAX_RETRIES,
} from "@/constants";
import type { TrafficSnapshot, Connection } from "@/types";

export type WsConnectionStatus = "connecting" | "connected" | "disconnected" | "error";

// ============================================================
// 泛型 WebSocket 客户端
// ============================================================

interface WsClientOptions<T> {
  url: string;
  onMessage: (data: T) => void;
  onStatusChange?: (status: WsConnectionStatus) => void;
}

class WsClient<T> {
  private ws: WebSocket | null = null;
  private retryCount = 0;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private stopped = false;
  private readonly options: WsClientOptions<T>;

  constructor(options: WsClientOptions<T>) {
    this.options = options;
  }

  connect(): void {
    if (this.stopped) return;
    this.options.onStatusChange?.("connecting");

    const ws = new WebSocket(this.options.url);
    this.ws = ws;

    ws.onopen = () => {
      this.retryCount = 0;
      this.options.onStatusChange?.("connected");
    };

    ws.onmessage = (event: MessageEvent) => {
      try {
        const data = JSON.parse(event.data as string) as T;
        this.options.onMessage(data);
      } catch {
        // 忽略非 JSON 消息
      }
    };

    ws.onclose = () => {
      if (this.stopped) return;
      this.options.onStatusChange?.("disconnected");
      this.scheduleReconnect();
    };

    ws.onerror = () => {
      this.options.onStatusChange?.("error");
    };
  }

  /** 主动断开，不再自动重连 */
  disconnect(): void {
    this.stopped = true;
    if (this.retryTimer) clearTimeout(this.retryTimer);
    this.ws?.close();
    this.ws = null;
  }

  /** 恢复自动重连 */
  resume(): void {
    this.stopped = false;
    if (!this.ws || this.ws.readyState === WebSocket.CLOSED) {
      this.connect();
    }
  }

  private scheduleReconnect(): void {
    if (this.retryCount >= WS_RECONNECT_MAX_RETRIES) {
      this.options.onStatusChange?.("error");
      return;
    }
    // 指数退避
    const delay = Math.min(
      WS_RECONNECT_DELAY_MS * Math.pow(2, this.retryCount),
      WS_RECONNECT_MAX_DELAY_MS
    );
    this.retryCount++;
    this.retryTimer = setTimeout(() => this.connect(), delay);
  }
}

// ============================================================
// 具体数据流实例（单例，全局共用）
// ============================================================

type TrafficCallback = (data: TrafficSnapshot) => void;
type ConnectionsCallback = (data: { connections: Connection[] }) => void;
type LogCallback = (line: string) => void;

let trafficCallbacks: TrafficCallback[] = [];
let connectionsCallbacks: ConnectionsCallback[] = [];
let logCallbacks: LogCallback[] = [];

let trafficClient: WsClient<TrafficSnapshot> | null = null;
let connectionsClient: WsClient<{ connections: Connection[] }> | null = null;
let logClient: WsClient<{ type: string; payload: string }> | null = null;

/** 订阅实时流量数据 */
export function subscribeTraffic(cb: TrafficCallback): () => void {
  trafficCallbacks.push(cb);
  if (!trafficClient) {
    trafficClient = new WsClient<TrafficSnapshot>({
      url: WS_PATH_TRAFFIC,
      onMessage: (data) => trafficCallbacks.forEach((fn) => fn(data)),
    });
    trafficClient.connect();
  }
  return () => {
    trafficCallbacks = trafficCallbacks.filter((fn) => fn !== cb);
  };
}

/** 订阅活动连接列表 */
export function subscribeConnections(
  cb: ConnectionsCallback
): () => void {
  connectionsCallbacks.push(cb);
  if (!connectionsClient) {
    connectionsClient = new WsClient({
      url: WS_PATH_CONNECTIONS,
      onMessage: (data) => connectionsCallbacks.forEach((fn) => fn(data)),
    });
    connectionsClient.connect();
  }
  return () => {
    connectionsCallbacks = connectionsCallbacks.filter((fn) => fn !== cb);
  };
}

/** 订阅内核日志流（原始日志，Audit 极客后门） */
export function subscribeLog(cb: LogCallback): () => void {
  logCallbacks.push(cb);
  if (!logClient) {
    logClient = new WsClient({
      url: WS_PATH_LOGS,
      onMessage: (data) => logCallbacks.forEach((fn) => fn(data.payload)),
    });
    logClient.connect();
  }
  return () => {
    logCallbacks = logCallbacks.filter((fn) => fn !== cb);
  };
}

/** 断开所有 WebSocket 连接（应用最小化到托盘时调用） */
export function disconnectAll(): void {
  trafficClient?.disconnect();
  connectionsClient?.disconnect();
  logClient?.disconnect();
}

/** 恢复所有 WebSocket 连接（应用从托盘恢复时调用） */
export function resumeAll(): void {
  trafficClient?.resume();
  connectionsClient?.resume();
  logClient?.resume();
}
