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

import { attachConsole } from "@tauri-apps/plugin-log";

// 仅在开发环境下自动将 Rust 日志绑定到 Webview Console
if (import.meta.env.DEV) {
  attachConsole().catch((err) => console.error("绑定 Tauri 日志控制台失败:", err));
}

const app = createApp(App);

app.use(createPinia());
app.use(router);

app.mount("#app");
