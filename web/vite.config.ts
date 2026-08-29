import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { loadEnv } from "vite";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, ".", "");
  const canonicalTarget = env.CENTAUR_CONTEXT_DEV_API_TARGET;
  const legacyTarget = env.CENTAUR_OS_DEV_API_TARGET;
  if (canonicalTarget && legacyTarget && canonicalTarget !== legacyTarget) {
    throw new Error("CENTAUR_CONTEXT_DEV_API_TARGET conflicts with legacy CENTAUR_OS_DEV_API_TARGET");
  }
  const apiTarget = canonicalTarget || legacyTarget || "http://127.0.0.1:8080";
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
