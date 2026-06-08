import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./styles.css";

async function bootstrap() {
  if (import.meta.env.VITE_BRAWLER_BROWSER_SMOKE === "1") {
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
