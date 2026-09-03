/**
 * Auroweave 前端入口
 * 作者: TanXiang
 */
import { createApp } from "vue";
import { createPinia } from "pinia";
import router from "@/router";
import App from "./App.vue";

// Design Tokens（必须最先加载）
import "@/styles/tokens.css";
// 全局公共样式（依赖 tokens.css 变量，须在 tokens 之后加载）
import "@/styles/common.css";
import "@/styles/panel.css";
import "@/styles/layout.css";

import { attachConsole, error as logError, warn as logWarn } from "@tauri-apps/plugin-log";

// 仅在开发环境下自动将 Rust 日志绑定到 Webview Console
if (import.meta.env.DEV) {
  attachConsole().catch((err) => console.error("绑定 Tauri 日志控制台失败:", err));
}

const app = createApp(App);

// ==================== 全局错误兜底 ====================
// 未捕获的渲染层异常：console 留痕 + 尽力转发到 Tauri 日志（plugin-log 可用时），
// 避免错误被 Vue 静默吞掉导致难以排查
app.config.errorHandler = (err, _instance, info) => {
  const detail = err instanceof Error ? `${err.name}: ${err.message}\n${err.stack}` : String(err);
  console.error(`[Vue ErrorHandler] ${info}:`, err);
  logError(`[Vue] ${info}: ${detail}`).catch(() => {
    // plugin-log 不可用（如纯浏览器环境）时仅保留 console 记录
  });
};

// 未处理的 Promise 拒绝：警告级记录（大量场景为可容忍的竞态拒绝）
window.addEventListener("unhandledrejection", (event) => {
  const reason = event.reason instanceof Error ? `${event.reason.name}: ${event.reason.message}` : String(event.reason);
  console.warn("[UnhandledRejection]", event.reason);
  logWarn(`[UnhandledRejection] ${reason}`).catch(() => {});
});

app.use(createPinia());
app.use(router);

app.mount("#app");
