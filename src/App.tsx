import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  BookOpenText,
  Building2,
  FileText,
  Inbox,
  Moon,
  RefreshCw,
  Search,
  Settings,
  Sun,
  Video,
} from "lucide-react";

type Theme = "dark" | "light" | "system";

type HealthResponse = {
  status: string;
  version: string;
};

const sections = [
  { label: "Inbox", icon: Inbox, active: true },
  { label: "Companies", icon: Building2 },
  { label: "Notebooks", icon: BookOpenText },
  { label: "Transcripts", icon: Video },
  { label: "Sources", icon: Activity },
  { label: "Settings", icon: Settings },
];

const feedItems = [
  {
    company: "GPW:CDR",
    type: "Official report",
    source: "GPW ESPI/EBI",
    time: "09:12",
    title: "Current report placeholder for watchlist company",
    unread: true,
  },
  {
    company: "GPW:PKN",
    type: "News",
    source: "Fixture feed",
    time: "Yesterday",
    title: "Fixture item proving the inbox layout can scan dense rows",
    unread: false,
  },
  {
    company: "NASDAQ:MSFT",
    type: "Transcript",
    source: "Local fixture",
    time: "Mon",
    title: "Transcript-derived note candidate waits for future provider work",
    unread: false,
  },
];

function resolveTheme(theme: Theme) {
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }

  return theme;
}

export function App() {
  const [theme, setTheme] = useState<Theme>(() => {
    return (localStorage.getItem("brawler.theme") as Theme | null) ?? "dark";
  });
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);

  const effectiveTheme = useMemo(() => resolveTheme(theme), [theme]);

  useEffect(() => {
    document.documentElement.dataset.theme = effectiveTheme;
    localStorage.setItem("brawler.theme", theme);
  }, [effectiveTheme, theme]);

  useEffect(() => {
    invoke<HealthResponse>("health")
      .then((response) => {
        setHealth(response);
        setHealthError(null);
      })
      .catch((error) => {
        setHealth(null);
        setHealthError(String(error));
      });
  }, []);

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand">
          <div className="brand-mark">B</div>
          <div>
            <div className="brand-title">Brawler</div>
            <div className="brand-subtitle">codename</div>
          </div>
        </div>

        <nav className="nav-list">
          {sections.map((section) => {
            const Icon = section.icon;
            return (
              <button
                className={section.active ? "nav-item nav-item-active" : "nav-item"}
                key={section.label}
                type="button"
                title={section.label}
              >
                <Icon size={18} aria-hidden="true" />
                <span>{section.label}</span>
              </button>
            );
          })}
        </nav>

        <div className="sidebar-footer">
          <div className="status-pill" title="Rust command boundary health">
            <span className={health ? "status-dot status-ok" : "status-dot status-warn"} />
            {health ? `${health.status} ${health.version}` : "health pending"}
          </div>
        </div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div className="search-box">
            <Search size={18} aria-hidden="true" />
            <span>Search companies, feed, notes</span>
          </div>
          <div className="topbar-actions">
            <button className="icon-button" type="button" title="Refresh sources">
              <RefreshCw size={18} />
            </button>
            <label className="theme-control" title="Theme">
              {effectiveTheme === "dark" ? <Moon size={16} /> : <Sun size={16} />}
              <select value={theme} onChange={(event) => setTheme(event.target.value as Theme)}>
                <option value="dark">Dark</option>
                <option value="light">Light</option>
                <option value="system">System</option>
              </select>
            </label>
          </div>
        </header>

        <section className="content-grid">
          <section className="feed-panel" aria-labelledby="inbox-title">
            <div className="panel-header">
              <div>
                <h1 id="inbox-title">Inbox</h1>
                <p>Fixture feed for the first desktop shell milestone.</p>
              </div>
              <div className="segmented-control" aria-label="Feed filter">
                <button type="button" className="segment-active">All</button>
                <button type="button">Unread</button>
                <button type="button">Saved</button>
              </div>
            </div>

            <div className="feed-list">
              {feedItems.map((item) => (
                <article className={item.unread ? "feed-row unread" : "feed-row"} key={item.title}>
                  <div className="feed-row-main">
                    <div className="feed-meta">
                      <span>{item.company}</span>
                      <span>{item.type}</span>
                      <span>{item.source}</span>
                      <span>{item.time}</span>
                    </div>
                    <h2>{item.title}</h2>
                  </div>
                  {item.unread ? <span className="unread-dot" title="Unread" /> : null}
                </article>
              ))}
            </div>
          </section>

          <aside className="detail-pane" aria-label="Feed item details">
            <div className="detail-icon">
              <FileText size={24} />
            </div>
            <h2>Selected item</h2>
            <p>
              The first scaffold keeps data local and uses fixture content until the SQLite and GPW
              milestones arrive.
            </p>
            <dl>
              <div>
                <dt>Source</dt>
                <dd>GPW ESPI/EBI</dd>
              </div>
              <div>
                <dt>Provenance</dt>
                <dd>Required before note creation</dd>
              </div>
              <div>
                <dt>AI mode</dt>
                <dd>source_grounded</dd>
              </div>
            </dl>
            {healthError ? <p className="error-text">Health command failed: {healthError}</p> : null}
          </aside>
        </section>
      </main>
    </div>
  );
}
