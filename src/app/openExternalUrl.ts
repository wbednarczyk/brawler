import { openUrl } from "@tauri-apps/plugin-opener";

/** Opens a URL in the system browser, swallowing the failure into a console
 * warning (no user-facing error surface for this — a dead/blocked external
 * link is not a Brawler-side failure). Shared across AppStateRoot's several
 * "open the source in the browser" callers (Today's evidence links, research
 * evidence, Inbox). */
export function openExternalUrl(url: string) {
  void openUrl(url).catch((error) => {
    console.error("Failed to open external URL", error);
  });
}
