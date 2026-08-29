import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { loadEnv } from "vite";

export default defineConfig(({ mode }) => {
  const apiTarget = loadEnv(mode, ".", "").CENTAUR_OS_DEV_API_TARGET || "http://127.0.0.1:8080";
  return {
    plugins: [react()],
    test: {
      environment: "jsdom",
      setupFiles: "./src/test-setup.ts",
      restoreMocks: true,
    },
    server: {
      proxy: {
        "/api": apiTarget,
        "/healthz": apiTarget,
        "/readyz": apiTarget,
      },
    },
  };
});
