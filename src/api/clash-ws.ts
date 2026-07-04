/**
 * WebSocket 客户端封装 (兼容 Sing-box / ClashAPI 接口规范)
 * 作者: TanXiang
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

    try {
      const ws = new WebSocket(this.options.url);
      this.ws = ws;

      ws.onopen = () => {
        this.retryCount = 0;
        this.options.onStatusChange?.("connected");
      };

      ws.onmessage = (event: MessageEvent) => {
        try {
          const raw = JSON.parse(event.data as string);
          this.options.onMessage(raw as T);
        } catch {
          // 忽略格式解析异常
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
    } catch {
      this.scheduleReconnect();
    }
  }

  disconnect(): void {
    this.stopped = true;
    if (this.retryTimer) clearTimeout(this.retryTimer);
    this.ws?.close();
    this.ws = null;
  }

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
    const delay = Math.min(
      WS_RECONNECT_DELAY_MS * Math.pow(2, this.retryCount),
      WS_RECONNECT_MAX_DELAY_MS
    );
    this.retryCount++;
    this.retryTimer = setTimeout(() => this.connect(), delay);
  }
}

// ------------------------------------------------------------
// 单例管理
// ------------------------------------------------------------
type TrafficCallback = (data: TrafficSnapshot) => void;
type ConnectionsCallback = (data: { connections: Connection[] }) => void;
type LogCallback = (line: string) => void;

let trafficCallbacks: TrafficCallback[] = [];
let connectionsCallbacks: ConnectionsCallback[] = [];
let logCallbacks: LogCallback[] = [];

let trafficClient: WsClient<any> | null = null;
let connectionsClient: WsClient<{ connections: Connection[] }> | null = null;
let logClient: WsClient<{ type: string; payload: string }> | null = null;

export function subscribeTraffic(cb: TrafficCallback): () => void {
  trafficCallbacks.push(cb);
  if (!trafficClient) {
    trafficClient = new WsClient<any>({
      url: WS_PATH_TRAFFIC,
      onMessage: (raw) => {
        // 兼容 Sing-box 的 { up: number, down: number } 与标准的 TrafficSnapshot 格式
        const snapshot: TrafficSnapshot = {
          download_speed: raw.down ?? raw.download_speed ?? 0,
          upload_speed: raw.up ?? raw.upload_speed ?? 0,
          total_download: raw.total_download ?? 0,
          total_upload: raw.total_upload ?? 0,
          active_connections: raw.active_connections ?? 0,
        };
        trafficCallbacks.forEach((fn) => fn(snapshot));
      },
    });
    trafficClient.connect();
  }
  return () => {
    trafficCallbacks = trafficCallbacks.filter((fn) => fn !== cb);
  };
}

export function subscribeConnections(cb: ConnectionsCallback): () => void {
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

export function disconnectAll(): void {
  trafficClient?.disconnect();
  connectionsClient?.disconnect();
  logClient?.disconnect();
}

export function resumeAll(): void {
  trafficClient?.resume();
  connectionsClient?.resume();
  logClient?.resume();
}
