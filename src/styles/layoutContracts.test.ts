import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const stylesDir = join(process.cwd(), "src", "styles");

function readStyle(relativePath: string) {
  return readFileSync(join(stylesDir, relativePath), "utf8");
}

const companiesCss = readStyle("screens/companies.css");
const controlsCss = readStyle("controls.css");
const eventsCss = readStyle("screens/events.css");
const inboxCss = readStyle("screens/inbox.css");
const layoutCss = readStyle("layout.css");
const notebooksCss = readStyle("screens/notebooks.css");
const researchCss = [
  readStyle("screens/research-layout.css"),
  readStyle("screens/research-components.css"),
  readStyle("screens/research-responsive.css"),
].join("\n");
const responsiveCss = readStyle("responsive.css");
const shellCss = readStyle("shell.css");
const sourcesCss = readStyle("screens/sources.css");
const uiCss = readStyle("ui.css");
const watchlistsCss = readStyle("screens/watchlists.css");

function ruleFor(css: string, selector: string) {
  const rules = [...css.matchAll(/(?<selectors>[^{}]+)\{(?<body>[^}]*)\}/gm)]
    .filter((match) =>
      match.groups?.selectors
        .split(",")
        .map((item) => item.trim())
        .includes(selector),
    )
    .map((match) => match.groups?.body ?? "");

  return rules.join("\n");
}

function mediaBlock(css: string, query: string) {
  const start = css.indexOf(`@media ${query}`);
  if (start === -1) {
    return "";
  }
  const nextMedia = css.indexOf("@media ", start + 1);
  return nextMedia === -1 ? css.slice(start) : css.slice(start, nextMedia);
}

describe("layout scroll contracts", () => {
  it("keeps app chrome fixed and routes overflow into workspace panels", () => {
    const appShellRule = ruleFor(shellCss, ".app-shell");
    const sidebarRule = ruleFor(shellCss, ".sidebar");
    const appMainRule = ruleFor(shellCss, ".app-main");
    const workspaceRule = ruleFor(shellCss, ".workspace");
    const feedPanelRule = ruleFor(layoutCss, ".feed-panel");

    expect(appShellRule).toContain("height: 100dvh");
    expect(appShellRule).toContain("min-height: 0");
    expect(appShellRule).toContain("overflow: hidden");
    // Navigation is a persistent left-sidebar IA spine (ADR 0054): the shell is
    // a two-column grid (sidebar | main). The sidebar scrolls vertically; the
    // main column routes overflow into workspace panels.
    expect(appShellRule).toContain("grid-template-columns: minmax(0, auto) minmax(0, 1fr)");
    expect(sidebarRule).toContain("overflow-y: auto");
    expect(appMainRule).toContain("grid-template-rows: auto minmax(0, 1fr)");
    expect(appMainRule).toContain("overflow: hidden");
    expect(workspaceRule).toContain("grid-template-rows: minmax(0, 1fr)");
    expect(workspaceRule).toContain("overflow: hidden");
    expect(feedPanelRule).toContain("height: 100%");
    expect(feedPanelRule).toContain("overflow: hidden");
  });

  it("keeps the Companies list as the primary flexible scroll region", () => {
    const layoutRule = ruleFor(companiesCss, ".companies-layout");
    const listRule = ruleFor(companiesCss, ".company-list");

    expect(layoutRule).toContain("grid-template-rows: auto auto minmax(520px, 1fr) auto auto");
    expect(layoutRule).toContain("overflow: hidden");
    expect(listRule).toContain("min-height: 0");
    expect(listRule).toContain("overflow: auto");
    expect(listRule).toContain("overscroll-behavior: contain");
  });

  it("keeps Inbox feed and detail panes as independent scroll regions", () => {
    const contentGridRule = ruleFor(shellCss, ".content-grid");
    const panelHeaderRule = ruleFor(uiCss, ".ui-panel-header");
    const feedPanelRule = ruleFor(layoutCss, ".feed-panel");
    const feedListRule = ruleFor(controlsCss, ".feed-list");
    const filterResetRule = ruleFor(controlsCss, ".filter-reset-row");
    const detailPaneRule = ruleFor(inboxCss, ".detail-pane");

    // Inbox places the feed and detail panes side by side (ADR 0047): feed
    // minmax(0,1fr), a 10px resizer, and a percentage detail track (50% default).
    expect(contentGridRule).toContain("grid-template-columns: minmax(0, 1fr) 10px var(--detail-pane-width, 50%)");
    expect(contentGridRule).toContain("overflow: auto hidden");
    expect(panelHeaderRule).toContain("flex-wrap: wrap");
    expect(feedPanelRule).toContain("display: flex");
    expect(feedPanelRule).toContain("flex-direction: column");
    expect(feedPanelRule).toContain("overflow: hidden");
    expect(feedListRule).toContain("flex: 1");
    expect(feedListRule).toContain("min-height: 0");
    expect(feedListRule).toContain("overflow: auto");
    expect(filterResetRule).toContain("flex-wrap: wrap");
    expect(detailPaneRule).toContain("height: 100%");
    expect(detailPaneRule).toContain("min-height: 0");
    // The rail scrolls vertically but never horizontally: a fixed-width grid
    // track must contain its content (wrap/ellipsis) rather than scroll or clip.
    expect(detailPaneRule).toContain("overflow-y: auto");
    expect(detailPaneRule).toContain("overflow-x: hidden");
  });

  it("keeps Watchlists member companies in a scrollable remaining-height region", () => {
    const workspaceRule = ruleFor(watchlistsCss, ".watchlists-workspace");
    const sidebarRule = ruleFor(watchlistsCss, ".watchlists-sidebar");
    const listRule = ruleFor(watchlistsCss, ".watchlist-list");
    const detailRule = ruleFor(watchlistsCss, ".watchlist-detail");
    const membersRule = ruleFor(watchlistsCss, ".watchlist-members-section");
    const tableRule = ruleFor(watchlistsCss, ".watchlist-member-table");

    expect(workspaceRule).toContain("min-height: 0");
    expect(workspaceRule).toContain("flex: 1");
    expect(sidebarRule).toContain("grid-template-rows: auto minmax(0, 1fr)");
    expect(listRule).toContain("min-height: 0");
    expect(listRule).toContain("overflow: auto");
    expect(detailRule).toContain("display: flex");
    expect(detailRule).toContain("flex-direction: column");
    expect(detailRule).toContain("overflow: hidden");
    expect(membersRule).toContain("grid-template-rows: auto minmax(0, 1fr)");
    expect(membersRule).toContain("min-height: 0");
    expect(membersRule).toContain("flex: 1");
    expect(tableRule).toContain("overflow: auto");
  });

  it("lets the fractional Inbox grid adapt at narrow widths without a track override", () => {
    // The Inbox content-grid now uses fractional 50/50 columns (minmax(0,1fr) +
    // a percentage detail track) that adapt to width on their own, and there is
    // no left sidebar to fit beside (ADR 0047). The old ~1120px track-shrink
    // override (which fixed the fixed-min overflow that clipped the detail pane)
    // is therefore removed — no .content-grid rule may live in this band, or it
    // would re-clobber the fractional default and single-column screens alike.
    const narrowBand = mediaBlock(responsiveCss, "(max-width: 1120px)");

    expect(narrowBand).not.toContain(".content-grid:not(.content-grid-single) {");
    expect(narrowBand).not.toContain(".content-grid {");
  });

  it("keeps the left-sidebar IA spine and 50/50 Inbox split (ADR 0054)", () => {
    const appShellRule = ruleFor(shellCss, ".app-shell");
    const sidebarRule = ruleFor(shellCss, ".sidebar");
    const navListRule = ruleFor(shellCss, ".nav-list");
    const contentGridRule = ruleFor(shellCss, ".content-grid");

    // Primary nav is a persistent left sidebar (ADR 0054), not a stacked top row:
    // the shell defines a column track for it and the nav stacks vertically.
    expect(appShellRule).toContain("grid-template-columns");
    expect(sidebarRule).toContain("border-right: 1px solid var(--border)");
    expect(navListRule).toContain("flex-direction: column");
    // The spine is a fixed-width rail (no user-resizable --sidebar-width var yet).
    expect(shellCss).not.toContain(".sidebar-resizer");
    // Inbox splits feed/detail side by side, 50/50 by default (percentage track).
    expect(contentGridRule).toContain("var(--detail-pane-width, 50%)");
  });

  it("keeps Notebooks subpanels independently scrollable on desktop", () => {
    const screenRule = ruleFor(notebooksCss, ".notebooks-screen");
    const companiesRule = ruleFor(notebooksCss, ".notebooks-company-nav");
    const mainRule = ruleFor(notebooksCss, ".notebooks-main");
    const notesRule = ruleFor(notebooksCss, ".notebooks-notes-list");
    const detailRule = ruleFor(notebooksCss, ".notebooks-detail-pane");
    const responsiveNotebooksRule = ruleFor(responsiveCss, ".notebooks-screen");

    expect(screenRule).toContain("min-height: 0");
    expect(screenRule).toContain("overflow: hidden");
    expect(companiesRule).toContain("min-height: 0");
    expect(companiesRule).toContain("overflow: auto");
    expect(mainRule).toContain("grid-template-rows: auto auto auto minmax(0, 1fr)");
    expect(mainRule).toContain("overflow: hidden");
    expect(notesRule).toContain("min-height: 0");
    expect(notesRule).toContain("overflow: auto");
    expect(detailRule).toContain("min-height: 0");
    expect(detailRule).toContain("overflow: auto");
    expect(responsiveNotebooksRule).toContain("overflow: auto");
  });

  it("keeps Events headers and filters outside the scrollable event body", () => {
    const eventsLayoutRule = ruleFor(eventsCss, ".events-layout");
    const weekGridRule = ruleFor(eventsCss, ".event-week-grid");
    const dayRule = ruleFor(eventsCss, ".event-week-day");
    const dayBodyRule = ruleFor(eventsCss, ".event-week-day-body");

    expect(eventsLayoutRule).toContain("flex: 1");
    expect(eventsLayoutRule).toContain("min-height: 0");
    expect(eventsLayoutRule).toContain("overflow: auto");
    expect(weekGridRule).toContain("min-height: 0");
    expect(weekGridRule).toContain("min-width: 920px");
    expect(dayRule).toContain("grid-template-rows: auto minmax(0, 1fr)");
    expect(dayBodyRule).toContain("min-height: 0");
    expect(dayBodyRule).toContain("min-width: 0");
    expect(dayBodyRule).toContain("overflow: auto");
    expect(dayBodyRule).toContain("overscroll-behavior: contain");
  });

  it("keeps Sources category and source rows at intrinsic content height", () => {
    const layoutRule = ruleFor(sourcesCss, ".sources-layout");
    const groupRule = ruleFor(sourcesCss, ".source-group");
    const groupListRule = ruleFor(sourcesCss, ".source-group-list");
    const rowMainRule = ruleFor(sourcesCss, ".source-row-main");
    const titleRule = ruleFor(sourcesCss, ".source-title-line h2");
    const subtitleRule = ruleFor(sourcesCss, ".source-row p");
    const sourceChipListRule = ruleFor(sourcesCss, ".sources-layout .ui-chip-list");
    const selectedSourceRule = ruleFor(sourcesCss, ".sources-layout .source-row-selected");

    expect(layoutRule).toContain("align-content: start");
    expect(groupRule).toContain("align-content: start");
    expect(groupListRule).toContain("align-content: start");
    expect(rowMainRule).toContain("display: flex");
    expect(rowMainRule).toContain("align-items: center");
    expect(titleRule).toContain("white-space: nowrap");
    expect(titleRule).toContain("text-overflow: ellipsis");
    expect(subtitleRule).toContain("white-space: nowrap");
    expect(subtitleRule).toContain("text-overflow: ellipsis");
    expect(sourceChipListRule).toContain("margin-top: 0");
    expect(selectedSourceRule).toContain("border-color: color-mix(in srgb, var(--primary) 76%, var(--border))");
    expect(selectedSourceRule).toContain("linear-gradient");
    expect(selectedSourceRule).toContain("0 0 0 1px color-mix(in srgb, var(--primary) 24%, transparent)");
  });

  it("keeps Research usable in side-zone width windows", () => {
    const sideZoneRule = mediaBlock(researchCss, "(max-width: 1380px)");
    const narrowRule = mediaBlock(researchCss, "(max-width: 1040px)");

    expect(sideZoneRule).toContain(
      "grid-template-columns: minmax(0, 1fr) 8px minmax(320px, 0.9fr)",
    );
    expect(sideZoneRule).not.toContain(".research-main-layout > .research-resizer");
    expect(narrowRule).toContain("grid-template-columns: minmax(0, 1fr)");
    expect(narrowRule).toContain(".research-main-layout > .research-resizer");
    expect(narrowRule).toContain("display: none");
  });

  it("keeps shared subnav labels intrinsic and prevents horizontal scrolling", () => {
    const panelRule = ruleFor(uiCss, ".ui-panel");
    const panelHeaderRule = ruleFor(uiCss, ".ui-panel-header");
    const segmentedControlRule = ruleFor(controlsCss, ".segmented-control");
    const segmentedControlButtonRule = ruleFor(controlsCss, ".segmented-control button");
    const subnavRule = ruleFor(uiCss, ".ui-subnav");
    const itemRule = ruleFor(uiCss, ".ui-subnav-item");
    const itemLabelRule = ruleFor(uiCss, ".ui-subnav-item span");
    const activeRule = ruleFor(uiCss, ".ui-subnav-item-active");
    const chipRule = ruleFor(uiCss, ".ui-status-chip");
    const accentChipRule = ruleFor(uiCss, ".ui-status-chip-accent");
    const pillRule = ruleFor(uiCss, ".ui-status-pill");
    const dangerPillRule = ruleFor(uiCss, ".ui-status-pill-danger");
    const chipListRule = ruleFor(uiCss, ".ui-chip-list");
    const denseRowRule = ruleFor(uiCss, ".ui-dense-row");
    const interactiveDenseRowRule = ruleFor(uiCss, ".ui-dense-row-interactive");
    const selectedDenseRowRule = ruleFor(uiCss, ".ui-dense-row-selected");
    const unreadDenseRowRule = ruleFor(uiCss, ".ui-dense-row-unread");
    const primaryButtonRule = ruleFor(uiCss, ".primary-button");
    const secondaryButtonRule = ruleFor(uiCss, ".secondary-button");
    const iconButtonRule = ruleFor(uiCss, ".icon-button");
    const dangerButtonRule = ruleFor(uiCss, ".danger-button");
    const compactButtonRule = ruleFor(uiCss, ".compact-button");
    const fieldClearRule = ruleFor(uiCss, ".field-clear-button");
    const emptyStateRule = ruleFor(uiCss, ".empty-state");
    const emptyStateActionsRule = ruleFor(uiCss, ".empty-state-actions");
    const sectionHeaderRule = ruleFor(uiCss, ".ui-section-header");
    const sectionHeaderAccentRule = ruleFor(uiCss, ".ui-section-header-accent");
    const sectionTitleRule = ruleFor(uiCss, ".ui-section-title");
    const sectionActionsRule = ruleFor(uiCss, ".ui-section-header-actions");
    const infoGridRule = ruleFor(uiCss, ".ui-info-grid");

    expect(panelRule).toContain("display: flex");
    expect(panelRule).toContain("overflow: hidden");
    expect(panelHeaderRule).toContain("flex-wrap: wrap");
    expect(segmentedControlRule).toContain("display: flex");
    expect(segmentedControlButtonRule).toContain("min-width: 72px");
    expect(subnavRule).toContain("width: max-content");
    expect(subnavRule).toContain("overflow-x: hidden");
    expect(subnavRule).toContain("overflow-y: auto");
    expect(itemRule).toContain("min-width: max-content");
    expect(itemLabelRule).toContain("white-space: nowrap");
    expect(activeRule).toContain("border-color: color-mix(in srgb, var(--primary) 52%, var(--border))");
    expect(chipRule).toContain("white-space: nowrap");
    expect(accentChipRule).toContain("color: var(--accent)");
    expect(pillRule).toContain("font-weight: 700");
    expect(dangerPillRule).toContain("color: var(--danger)");
    expect(chipListRule).toContain("flex-wrap: wrap");
    expect(denseRowRule).toContain("min-width: 0");
    expect(interactiveDenseRowRule).toContain("cursor: pointer");
    expect(selectedDenseRowRule).toContain("box-shadow: inset 3px 0 0 var(--primary)");
    expect(unreadDenseRowRule).toContain("font-weight: 650");
    expect(primaryButtonRule).toContain("background: color-mix(in srgb, var(--primary) 18%, var(--surface-raised))");
    expect(secondaryButtonRule).toContain("border-color: color-mix(in srgb, var(--primary) 34%, var(--border))");
    expect(iconButtonRule).toContain("width: 40px");
    expect(dangerButtonRule).toContain("color: var(--danger)");
    expect(compactButtonRule).toContain("min-height: 34px");
    expect(fieldClearRule).toContain("position: absolute");
    expect(fieldClearRule).toContain("border-radius: 999px");
    expect(emptyStateRule).toContain("justify-content: space-between");
    expect(emptyStateRule).toContain("background: var(--surface-raised)");
    expect(emptyStateActionsRule).toContain("justify-content: flex-end");
    expect(sectionHeaderRule).toContain("justify-content: space-between");
    expect(sectionHeaderRule).toContain("min-width: 0");
    expect(sectionHeaderAccentRule).toContain("var(--section-accent, var(--primary))");
    expect(sectionTitleRule).toContain("min-width: 0");
    expect(sectionActionsRule).toContain("margin-left: auto");
    expect(infoGridRule).toContain("min-width: 0");
  });
});
