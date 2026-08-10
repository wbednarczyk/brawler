import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./app/App";
import "./styles.css";

async function bootstrap() {
  if (import.meta.env.VITE_BRAWLER_BROWSER_SMOKE === "1") {
    // Build-freshness stamp (bug 2059fd8): expose the stamp this server was built
    // with so the Playwright global-setup can detect a reused STALE dev server
    // serving pre-rework code. Smoke-only — never present in the real app.
    (window as unknown as { __BRAWLER_BUILD_STAMP__?: string }).__BRAWLER_BUILD_STAMP__ =
      import.meta.env.VITE_BRAWLER_BUILD_STAMP;
    const { installBrowserSmokeRuntime } = await import("./test/browserSmokeRuntime");
    installBrowserSmokeRuntime();
  }

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void bootstrap();
