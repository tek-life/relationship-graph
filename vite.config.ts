import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  // Vite options tailored for Tauri development and only applied using `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust error messages during build
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    host: true, // 监听 0.0.0.0，供 Windows 浏览器与手机经局域网访问
    port: 1420,
    strictPort: true,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
