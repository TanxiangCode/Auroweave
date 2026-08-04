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

import { attachConsole } from "@tauri-apps/plugin-log";

// 仅在开发环境下自动将 Rust 日志绑定到 Webview Console
if (import.meta.env.DEV) {
  attachConsole().catch((err) => console.error("绑定 Tauri 日志控制台失败:", err));
}

const app = createApp(App);

app.use(createPinia());
app.use(router);

app.mount("#app");
