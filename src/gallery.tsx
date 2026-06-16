import React from "react";
import ReactDOM from "react-dom/client";
import { PrimitiveGallery } from "./ui/PrimitiveGallery";
import "./styles.css";

// Dev/preview-only entry (gallery.html). Not referenced from index.html, so it
// is never included in the production/Tauri build — it exists to view the UI
// primitive catalog and as the Playwright visual/overflow target.
ReactDOM.createRoot(document.getElementById("gallery-root") as HTMLElement).render(
  <React.StrictMode>
    <PrimitiveGallery />
  </React.StrictMode>,
);
