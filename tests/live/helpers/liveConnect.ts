import { chromium, type Browser, type BrowserContext, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { execSync } from "node:child_process";

// Connects Playwright to the REAL running Brawler app via WebView2's Chrome
// DevTools Protocol (ADR 0066). The app is launched separately — on Windows via
// `scripts/windows/dev-live.ps1`, which sets
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=<port>" — this
// helper only attaches to the already-open debug port.

const DEFAULT_PORT = 9222;
const DEFAULT_LOCALHOST_URL = `http://localhost:${DEFAULT_PORT}`;

/**
 * WSL2 without mirrored networking cannot always reach a Windows-host localhost
 * port directly (`localhost` in WSL resolves to the WSL VM, not Windows). The
 * documented workaround: read the Windows host's IP from the `nameserver` line
 * WSL writes into /etc/resolv.conf (that address routes to the Windows host in
 * NAT mode). This is best-effort — mirrored-networking WSL configurations
 * already reach `localhost` directly and never need this fallback.
 */
function windowsHostIpFromResolvConf(): string | null {
  try {
    const contents = readFileSync("/etc/resolv.conf", "utf8");
    const match = contents.match(/^nameserver\s+(\S+)/m);
    return match ? match[1] : null;
  } catch {
    return null;
  }
}

/**
 * On WSL with DNS tunneling the resolv.conf nameserver is a stub
 * (10.255.255.254), NOT the Windows host — the default-route gateway is the
 * real host-side vEthernet address. Probe it first (found live 2026-07-03).
 */
function windowsHostIpFromDefaultRoute(): string | null {
  try {
    const route = execSync("ip route show default", { encoding: "utf8", timeout: 3_000 });
    const match = route.match(/default via (\S+)/);
    return match ? match[1] : null;
  } catch {
    return null;
  }
}

async function canConnect(url: string): Promise<boolean> {
  try {
    const response = await fetch(`${url}/json/version`, {
      signal: AbortSignal.timeout(3_000),
    });
    return response.ok;
  } catch {
    return false;
  }
}

/**
 * Resolves the CDP endpoint to connect to: `BRAWLER_CDP_URL` if set, otherwise
 * `http://localhost:<port>`, falling back to the WSL-resolved Windows host IP
 * when localhost is unreachable (see `windowsHostIpFromResolvConf`).
 */
export async function resolveCdpUrl(port = DEFAULT_PORT): Promise<string> {
  const explicit = process.env.BRAWLER_CDP_URL;
  if (explicit) return explicit;

  const localhostUrl = `http://localhost:${port}`;
  if (await canConnect(localhostUrl)) return localhostUrl;

  const candidates = [windowsHostIpFromDefaultRoute(), windowsHostIpFromResolvConf()].filter(
    (ip, i, all): ip is string => ip !== null && all.indexOf(ip) === i,
  );
  for (const ip of candidates) {
    const hostUrl = `http://${ip}:${port}`;
    if (await canConnect(hostUrl)) return hostUrl;
  }

  throw new Error(
    `Could not reach a live Brawler CDP endpoint at ${localhostUrl}` +
      (candidates.length ? ` or ${candidates.map((ip) => `http://${ip}:${port}`).join(", ")}` : "") +
      `. Launch the app with scripts/windows/dev-live.ps1 -Port ${port}, or set BRAWLER_CDP_URL explicitly.`,
  );
}

export type LiveConnection = {
  browser: Browser;
  context: BrowserContext;
  page: Page;
};

/**
 * Connects to the live app and returns its existing browser context/page.
 * WebView2 exposes exactly one browser context (the app window) — this does
 * not create a new context/page the way a fresh browser launch would.
 */
export async function connectToLiveApp(port = 9222): Promise<LiveConnection> {
  const cdpUrl = await resolveCdpUrl(port);
  const browser = await chromium.connectOverCDP(cdpUrl);

  const context = browser.contexts()[0];
  if (!context) {
    throw new Error(`Connected to ${cdpUrl} but found no existing browser context (no app window open?).`);
  }

  const page = context.pages()[0] ?? (await context.waitForEvent("page"));
  return { browser, context, page };
}
