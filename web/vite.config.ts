import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { loadEnv, type Plugin } from "vite";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const composed = existsSync(path.resolve("context-composition.json"));
const webRoot = process.cwd();
const extensionRoot = path.join(webRoot, ".context-overlay", "views");
const extensionDependencies = composed && existsSync(path.join(webRoot, ".context-overlay/package.json"))
  ? Object.keys(JSON.parse(readFileSync(path.join(webRoot, ".context-overlay/package.json"), "utf8")).dependencies ?? {}) : [];
const extensionBoundary: Plugin = {
  name: "context-extension-import-boundary",
  enforce: "pre",
  resolveId(source, importer) {
    if (!importer?.startsWith(extensionRoot + path.sep)) return null;
    if (source === "/@react-refresh" || source.startsWith("\0") || source.startsWith(path.join(webRoot, ".context-overlay/node_modules") + path.sep)) return null;
    if (source === path.join(webRoot, "src/ui/index.ts") || ["react", "react-dom"].some(name => source === path.join(webRoot, "node_modules", name) || source.startsWith(path.join(webRoot, "node_modules", name) + "/"))) return null;
    if (source === "@centaur-context/ui") return path.join(webRoot, "src/ui/index.ts");
    if (source.startsWith(".")) {
      const target = path.resolve(path.dirname(importer), source.split("?")[0]);
      if (!target.startsWith(extensionRoot + path.sep)) throw new Error("External views cannot import outside their selected source root");
    } else if (source.startsWith("/") || source.startsWith("file:") || source.includes("web/src") || source.includes("\\")) {
      throw new Error("External views must use @centaur-context/ui and locked dependencies");
    }
    if (!source.startsWith(".") && !["react", "react-dom", ...extensionDependencies].some(name => source === name || source.startsWith(name + "/"))) throw new Error("External view dependency is not declared in its lock owner");
    return null;
  },
};

export default defineConfig(({ mode }) => {
  const env = composed ? process.env : { ...loadEnv(mode, ".", ""), ...process.env };
  const canonicalTarget = env.CENTAUR_CONTEXT_DEV_API_TARGET;
  const legacyTarget = env.CENTAUR_OS_DEV_API_TARGET;
  if (canonicalTarget && legacyTarget && canonicalTarget !== legacyTarget) {
    throw new Error("CENTAUR_CONTEXT_DEV_API_TARGET conflicts with legacy CENTAUR_OS_DEV_API_TARGET");
  }
  const apiTarget = canonicalTarget || legacyTarget || "http://127.0.0.1:8080";
  return {
    envDir: composed ? false : undefined,
    plugins: [extensionBoundary, react()],
    resolve: {
      alias: [
        { find: "@centaur-context/ui", replacement: path.join(webRoot, "src/ui/index.ts") },
        ...["react", "react-dom"].map(name => ({ find: new RegExp(`^${name}(/.*)?$`), replacement: path.join(webRoot, "node_modules", name) + "$1" })),
      ],
      dedupe: ["react", "react-dom"],
    },
    test: {
      environment: "jsdom",
      setupFiles: "./src/test-setup.ts",
      restoreMocks: true,
    },
    server: {
      fs: { strict: true, allow: [webRoot] },
      proxy: {
        "/api": apiTarget,
        "/healthz": apiTarget,
        "/readyz": apiTarget,
      },
    },
  };
});
