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

const app = createApp(App);

app.use(createPinia());
app.use(router);

app.mount("#app");
