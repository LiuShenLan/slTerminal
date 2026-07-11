import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // Tauri 本地加载，无需网络传输——提高 chunk 警告阈值
  build: {
    chunkSizeWarningLimit: 1024,
  },
  // 2. tauri expects a fixed port, fail if that port is not available
  // 3. 防止预构建时 ESBuild 损坏 dockview-core 的 Orientation 枚举
  optimizeDeps: {
    exclude: ["dockview-react", "dockview-core"],
  },
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
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
