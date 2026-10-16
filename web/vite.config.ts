import { readFileSync } from "node:fs";
import { defineConfig, type Plugin } from "vite";
import solid from "vite-plugin-solid";

// 앱 버전은 Cargo.toml 하나에서 관리한다. index.html의 %APP_VERSION%을 빌드 시 치환.
const cargoToml = readFileSync(new URL("../Cargo.toml", import.meta.url), "utf8");
const appVersion = /^version\s*=\s*"([^"]+)"/m.exec(cargoToml)?.[1];
if (!appVersion) {
  throw new Error("Cargo.toml에서 version을 찾지 못했습니다.");
}

function appVersionPlugin(): Plugin {
  return {
    name: "app-version",
    transformIndexHtml: (html) => html.replaceAll("%APP_VERSION%", appVersion),
  };
}

export default defineConfig({
  plugins: [solid(), appVersionPlugin()],
  base: "/key2gksrmf/",
  build: {
    target: "esnext",
  },
});
