/**
 * WebSocket 客户端封装 (兼容 Sing-box / ClashAPI 接口规范)
 * 作者: TanXiang
 */
import {
  WS_RECONNECT_DELAY_MS,
  WS_RECONNECT_MAX_DELAY_MS,
  WS_MAX_CONSECUTIVE_FAILURES,
} from "@/constants";
import type { TrafficSnapshot, Connection } from "@/types";
import { useSettingsStore } from "@/stores/settings.store";

export type WsConnectionStatus = "connecting" | "connected" | "disconnected" | "error";

interface WsClientOptions<T> {
  url: string | (() => string);
  onMessage: (data: T) => void;
  onStatusChange?: (status: WsConnectionStatus) => void;
}

class WsClient<T> {
  private ws: WebSocket | null = null;
  private retryCount = 0;
  private consecutiveFailures = 0;
  private consecutiveFailuresResetTime = 0;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private stopped = false;
  private resetTimer: ReturnType<typeof setInterval> | null = null;
  private readonly options: WsClientOptions<T>;

  constructor(options: WsClientOptions<T>) {
    this.options = options;
    // 启动定时器，每分钟检查并重置失败计数器（允许恢复尝试）
    // 重置后主动发起重连，确保 sing-box 启动后 WebSocket 能恢复连接
    this.resetTimer = setInterval(() => {
      if (this.consecutiveFailures > 0 && Date.now() - this.consecutiveFailuresResetTime > 60000) {
        console.warn("[WebSocket] 重置失败计数器:", this.consecutiveFailures, "-> 0");
        this.consecutiveFailures = 0;
        this.retryCount = 0;
        this.stopped = false;
        this.connect();
      }
    }, 60000);
  }

  connect(): void {
    if (this.stopped) return;

    // 清理旧连接：移除事件处理器并关闭，防止旧 onclose 触发多余的重连
    if (this.ws) {
      this.ws.onopen = null;
      this.ws.onmessage = null;
      this.ws.onclose = null;
      this.ws.onerror = null;
      this.ws.close();
      this.ws = null;
    }

    this.options.onStatusChange?.("connecting");

    try {
      const targetUrl = typeof this.options.url === "function" ? this.options.url() : this.options.url;
      const ws = new WebSocket(targetUrl);
      this.ws = ws;

      ws.onopen = () => {
        if (this.ws !== ws) return; // 忽略旧连接事件
        this.retryCount = 0;
        this.consecutiveFailures = 0;
        this.options.onStatusChange?.("connected");
      };

      ws.onmessage = (event: MessageEvent) => {
        if (this.ws !== ws) return; // 忽略旧连接事件
        try {
          const raw = JSON.parse(event.data as string);
          this.options.onMessage(raw as T);
        } catch {
          // 忽略格式解析异常
        }
      };

      ws.onclose = () => {
        if (this.ws !== ws) return; // 忽略旧连接事件
        if (this.stopped) return;
        this.options.onStatusChange?.("disconnected");
        this.scheduleReconnect();
      };

      ws.onerror = () => {
        if (this.ws !== ws) return; // 忽略旧连接事件
        this.options.onStatusChange?.("error");
      };
    } catch {
      this.scheduleReconnect();
    }
  }

  disconnect(): void {
    this.stopped = true;
    if (this.retryTimer) clearTimeout(this.retryTimer);
    if (this.resetTimer) clearInterval(this.resetTimer);
    // 移除事件处理器后关闭，防止异步 onclose 在 stopped 被重置后触发多余重连
    if (this.ws) {
      this.ws.onopen = null;
      this.ws.onmessage = null;
      this.ws.onclose = null;
      this.ws.onerror = null;
      this.ws.close();
      this.ws = null;
    }
    this.consecutiveFailures = 0;
  }

  resume(): void {
    this.stopped = false;
    this.consecutiveFailures = 0;
    this.retryCount = 0;
    // 重建 disconnect() 中被清除的 resetTimer，确保后续失败后仍能自动恢复
    if (!this.resetTimer) {
      this.resetTimer = setInterval(() => {
        if (this.consecutiveFailures > 0 && Date.now() - this.consecutiveFailuresResetTime > 60000) {
          console.warn("[WebSocket] 重置失败计数器:", this.consecutiveFailures, "-> 0");
          this.consecutiveFailures = 0;
          this.retryCount = 0;
          this.stopped = false;
          this.connect();
        }
      }, 60000);
    }
    if (!this.ws || this.ws.readyState === WebSocket.CLOSED) {
      this.connect();
    }
  }

  private scheduleReconnect(): void {
    if (this.stopped) return;
    // 超过最大连续失败次数后停止重连，避免无限重连耗尽资源
    if (this.consecutiveFailures >= WS_MAX_CONSECUTIVE_FAILURES) {
      this.stopped = true;
      this.options.onStatusChange?.("error");
      return;
    }
    const delay = Math.min(
      WS_RECONNECT_DELAY_MS * Math.pow(2, this.retryCount),
      WS_RECONNECT_MAX_DELAY_MS
    );
    this.retryCount++;
    this.consecutiveFailures++;
    this.consecutiveFailuresResetTime = Date.now();
    this.retryTimer = setTimeout(() => {
      this.connect();
    }, delay);
  }
}

// ------------------------------------------------------------
// 动态获取最新的 WebSocket URL，以在设置端口变化时生效
// ------------------------------------------------------------
function getDynamicWsUrl(path: string): string {
  try {
    const store = useSettingsStore();
    const port = store.settings.clash_api_port || 9090;
    return `ws://127.0.0.1:${port}${path}`;
  } catch {
    // 降级兜底
    return `ws://127.0.0.1:9090${path}`;
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
      url: () => getDynamicWsUrl("/traffic"),
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
      url: () => getDynamicWsUrl("/connections"),
      onMessage: (data: any) => {
        // 归一化转换 Clash / Sing-box 的原始 Connection 数据结构，匹配前端 interface Connection 定义
        const normalizedConns = (data.connections || []).map((conn: any) => {
          const metadata = conn.metadata || {};
          const chains = conn.chains || [];
          const destHost = metadata.host || metadata.destinationIP || "未知主机";
          const destPort = parseInt(metadata.destinationPort) || 0;
          const outboundNode = chains[chains.length - 1] || "direct";

          return {
            id: conn.id,
            destination: destHost,
            port: destPort,
            outbound: outboundNode,
            rule: conn.rule || "Match",
            upload_bytes: conn.upload || 0,
            download_bytes: conn.download || 0,
            start: conn.start ? new Date(conn.start).getTime() : Date.now(),
          } as Connection;
        });

        connectionsCallbacks.forEach((fn) => fn({ connections: normalizedConns }));
      },
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
      url: () => getDynamicWsUrl("/logs"),
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

export function reconnectAll(): void {
  trafficClient?.disconnect();
  connectionsClient?.disconnect();
  logClient?.disconnect();

  trafficClient?.resume();
  connectionsClient?.resume();
  logClient?.resume();
}
