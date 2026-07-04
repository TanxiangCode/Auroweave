/**
 * Vite 配置
 * 作者: TanXiang
 */
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "path";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],

  // 路径别名（@ → src/）
  resolve: {
    alias: {
      "@": resolve(__dirname, "src"),
    },
  },

  // Vite options tailored for Tauri development
  // 1. 防止 Vite 遮蔽 Rust 错误
  clearScreen: false,
  // 2. Tauri 需要固定端口
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. 忽略 src-tauri 目录变动
      ignored: ["**/src-tauri/**"],
    },
  },

  // 测试配置（vitest）
  test: {
    environment: "jsdom",
    include: ["tests/unit/**/*.{test,spec}.ts"],
  },
}));
