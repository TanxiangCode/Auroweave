/**
 * 路由配置
 * 作者: TanXiang
 *
 * 五个主视图与计划书第五章 views/ 目录一一对应
 */
import { createRouter, createWebHashHistory } from "vue-router";

const router = createRouter({
  // 使用 hash history，兼容 Tauri webview
  history: createWebHashHistory(),
  routes: [
    {
      path: "/",
      name: "dashboard",
      component: () => import("@/views/DashboardView.vue"),
      meta: { title: "Dashboard" },
    },
    {
      path: "/proxies",
      name: "proxies",
      component: () => import("@/views/ProxiesView.vue"),
      meta: { title: "代理节点" },
    },
    {
      path: "/routing",
      name: "routing",
      component: () => import("@/views/RoutingView.vue"),
      meta: { title: "分流配置" },
    },
    {
      path: "/audit",
      name: "audit",
      component: () => import("@/views/AuditView.vue"),
      meta: { title: "安全审计" },
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("@/views/SettingsView.vue"),
      meta: { title: "设置" },
    },
    // 未匹配路由重定向到 Dashboard
    {
      path: "/:pathMatch(.*)*",
      redirect: "/",
    },
  ],
});

export default router;
