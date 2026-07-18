import { test, expect, type Page } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// v0.57 BIG LIVE VERIFICATION pass #2 (today's build: skin-badge read-model fix,
// mgmt-holdings junk gate, KRU parser, insider startup catch-up, AUTOMATIC
// report-backfill, migrations 0096/0097, toast cap, statement_type 0095,
// health-facts backfill). Backend invoke = hard evidence; screenshots for the
// human charter. NON-MUTATING except the two explicitly-authorised state
// changes: ONE red-flag ack + ONE OCR confirm on SNT.

const SHOTS = "test-results/live/v057-2";
mkdirSync(SHOTS, { recursive: true });

let connection: LiveConnection;
let page: Page;
test.describe.configure({ mode: "serial" });

type Company = { id: string; ticker: string; qualifiedTicker: string };
async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return page.evaluate(
    async ({ cmd, args }) => {
      const internals = (window as unknown as Record<string, any>).__TAURI_INTERNALS__;
      return internals.invoke(cmd, args ?? {});
    },
    { cmd, args },
  );
}
async function tryInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T | { __err: string }> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    return { __err: String(e).slice(0, 200) };
  }
}

let companies: Company[] = [];
const byTicker = (t: string): Company | undefined => companies.find((c) => c.ticker === t);
const log = (s: string) => console.log(s);
const report: Record<string, unknown> = {};

test.beforeAll(async () => {
  connection = await connectToLiveApp();
  page = connection.page;
  companies = await invoke<Company[]>("list_companies");
  log(`companies tracked: ${companies.length}`);
});
test.afterAll(async () => {
  writeFileSync(`${SHOTS}/report.json`, JSON.stringify(report, null, 2));
  await connection.browser.close();
});

const TARGETS = ["CDR", "CBF", "ACP", "TXT", "BFT", "KGH", "ABE", "KRU", "SNT", "XTB", "PKO", "PZU"];

test("S3a — company health across targets", async () => {
  test.setTimeout(180_000);
  const rows: any[] = [];
  for (const tk of TARGETS) {
    const c = byTicker(tk);
    if (!c) { rows.push({ tk, missing: true }); continue; }
    const h = await tryInvoke<any>("get_company_health", { companyId: c.id });
    if ((h as any).__err) { rows.push({ tk, err: (h as any).__err }); continue; }
    const latest = (h as any)?.latest;
    rows.push({
      tk,
      fy: latest?.fiscalYear,
      stmt: (h as any)?.statementType,
      zState: latest?.altman?.state,
      z: latest?.altman?.zScore,
      band: latest?.altman?.band,
      fState: latest?.piotroski?.state,
      fScore: latest?.piotroski?.score,
      fMissing: (latest?.piotroski?.missing ?? []).map((m: any) => (typeof m === "string" ? m : m?.key)).join(","),
    });
  }
  report.health = rows;
  rows.forEach((r) => log(`HEALTH ${r.tk}: ${JSON.stringify(r)}`));
  // PKO/PZU/XTB must be not_applicable when latest present
  for (const tk of ["PKO", "PZU", "XTB"]) {
    const r = rows.find((x) => x.tk === tk);
    if (r && r.zState && r.zState !== "no-latest") {
      log(`FINANCIAL ${tk}: zState=${r.zState} (expect not_applicable)`);
    }
  }
});

test("S3b — red flags across watchlist + ack one on ABE", async () => {
  test.setTimeout(120_000);
  const allActive: any[] = [];
  let history = 0;
  for (const c of companies) {
    const v = await tryInvoke<any>("get_red_flags", { companyId: c.id });
    if ((v as any).__err) continue;
    ((v as any).active ?? []).forEach((f: any) =>
      allActive.push({ tk: c.ticker, id: f.flagId ?? f.id, type: f.flagType, sev: f.severity, title: f.title }),
    );
    history += ((v as any).history ?? []).length;
  }
  report.redFlagsActiveCount = allActive.length;
  report.redFlagsHistoryCount = history;
  log(`RED FLAGS active=${allActive.length} history=${history}`);
  allActive.slice(0, 40).forEach((r) => log(`  ${r.tk}: ${r.type} [${r.sev}] ${r.title} id=${r.id}`));

  // Ack ONE: prefer a fund_exit on ABE, else any flag on ABE, else any fund_exit anywhere.
  const abe = byTicker("ABE");
  let target: any = null;
  if (abe) {
    const abeView = await tryInvoke<any>("get_red_flags", { companyId: abe.id });
    const abeActive = (abeView as any)?.active ?? [];
    target = abeActive.find((f: any) => /fund_exit|fund-exit/i.test(f.flagType)) ?? abeActive[0];
    if (target) target.__company = abe;
  }
  if (!target) {
    const anyFundExit = allActive.find((f) => /fund_exit/i.test(f.type));
    const pick = anyFundExit ?? allActive[0];
    if (pick) target = { flagId: pick.id, flagType: pick.type, title: pick.title, __company: byTicker(pick.tk) };
  }
  if (!target) { log("ACK: no active red flag to acknowledge"); report.ack = "none-available"; return; }
  const flagId = target.flagId ?? target.id;
  log(`ACK target: ${target.__company?.ticker} ${target.flagType} "${target.title}" id=${flagId}`);
  const before = await invoke<any>("get_red_flags", { companyId: target.__company.id });
  const after = await invoke<any>("acknowledge_red_flag", { input: { flagId } });
  const stillActive = (after.active ?? []).some((f: any) => (f.flagId ?? f.id) === flagId);
  const nowInHistory = (after.history ?? []).some((f: any) => (f.flagId ?? f.id) === flagId);
  report.ack = {
    company: target.__company?.ticker, flagId, flagType: target.flagType,
    beforeActive: (before.active ?? []).length, afterActive: (after.active ?? []).length,
    stillActive, nowInHistory,
  };
  log(`ACK result: stillActive=${stillActive} nowInHistory=${nowInHistory} active ${(before.active ?? []).length}->${(after.active ?? []).length}`);
  expect(stillActive, "acked flag must leave active").toBe(false);
});

test("S3c — ownership skin badge + junk-row absence", async () => {
  test.setTimeout(120_000);
  const rows: any[] = [];
  let junkFound: string[] = [];
  for (const tk of ["ABE", "KRU", "ACP", "MDV", "DVL", "DIG", "CBF", "TXT"]) {
    const c = byTicker(tk);
    if (!c) continue;
    const ov = await tryInvoke<any>("get_ownership_overview", { companyId: c.id });
    if ((ov as any).__err) { rows.push({ tk, err: (ov as any).__err }); continue; }
    const holders = (ov as any).holders ?? [];
    const mgmt = (ov as any).managementHoldings ?? (ov as any).management ?? [];
    const skins = holders.filter((h: any) => h.skinInTheGame);
    rows.push({
      tk, holders: holders.length, mgmtRows: Array.isArray(mgmt) ? mgmt.length : undefined,
      skins: skins.map((h: any) => `${h.name}:${h.skinInTheGame?.person}${h.skinInTheGame?.via ? `/via ${h.skinInTheGame.via}` : ""}`),
    });
    // junk gate: management holdings must not contain fund-class junk like "UL EUROPEJSKA"
    const flat = JSON.stringify(mgmt).toUpperCase();
    if (/EUROPEJSKA|OFE|TFI|FUNDUSZ INWESTYCYJNY/.test(flat)) junkFound.push(`${tk}: ${flat.slice(0, 120)}`);
  }
  report.ownership = rows;
  report.mgmtJunkFound = junkFound;
  rows.forEach((r) => log(`OWNERSHIP ${r.tk}: holders=${r.holders} mgmt=${r.mgmtRows} skins=[${(r.skins ?? []).join(" | ")}]`));
  log(junkFound.length ? `MGMT JUNK PRESENT: ${junkFound.join("; ")}` : "MGMT JUNK: none detected");
});

test("S3d — insider overview populated without manual refresh", async () => {
  test.setTimeout(120_000);
  let withTx = 0, withHoldings = 0;
  const examples: string[] = [];
  for (const c of companies) {
    const o = await tryInvoke<any>("get_insider_overview", { companyId: c.id });
    if ((o as any).__err) continue;
    const txs = (o as any).transactions?.length ?? 0;
    const holdings = (o as any).holdings?.length ?? 0;
    if (txs > 0) { withTx++; examples.push(`${c.ticker}: tx=${txs} w90=${(o as any).window90d?.state} w12=${(o as any).window12m?.state}`); }
    if (holdings > 0) withHoldings++;
  }
  report.insider = { withTx, withHoldings, examples: examples.slice(0, 25) };
  log(`INSIDER: withTx=${withTx} withHoldings=${withHoldings}`);
  examples.slice(0, 25).forEach((e) => log(`  ${e}`));
});

test("S3e — scorecard evaluate resolves piotroski_f/altman_z", async () => {
  test.setTimeout(120_000);
  const fws = await tryInvoke<any[]>("list_quality_frameworks");
  const fwId = Array.isArray(fws) && fws[0]?.id;
  report.scorecardFramework = fwId;
  if (!fwId) { log("SCORECARD: no framework"); return; }
  const results: any[] = [];
  for (const tk of ["ACP", "TXT", "KGH"]) {
    const c = byTicker(tk);
    if (!c) continue;
    const ev = await tryInvoke<any>("evaluate_framework", { input: { frameworkId: fwId, companyId: c.id } });
    if ((ev as any).__err) { results.push({ tk, err: (ev as any).__err }); continue; }
    const crits = (ev as any).criteria ?? (ev as any).results ?? [];
    const keyed = crits.map((cr: any) => ({
      key: cr.metricKey ?? cr.key ?? cr.expression,
      state: cr.state ?? cr.status,
      value: cr.value ?? cr.resolvedValue,
    }));
    const piotroski = keyed.find((k: any) => /piotroski/i.test(String(k.key)));
    const altman = keyed.find((k: any) => /altman/i.test(String(k.key)));
    results.push({ tk, criteria: keyed.length, piotroski, altman });
  }
  report.scorecard = results;
  results.forEach((r) => log(`SCORECARD ${r.tk}: ${JSON.stringify(r)}`));
});

test("S4 — OCR happy path on SNT", async () => {
  test.setTimeout(360_000);
  const settings = await tryInvoke<any>("get_settings");
  const pool = (settings as any)?.capabilityProviders?.vision_extraction ?? [];
  report.ocrVisionProviders = pool.length;
  log(`OCR vision providers configured = ${pool.length}`);
  const snt = byTicker("SNT");
  if (!snt) { log("OCR: SNT not tracked"); return; }
  const before = await tryInvoke<any>("get_ownership_overview", { companyId: snt.id });
  const residualsBefore = (before as any)?.residuals?.length ?? 0;
  log(`OCR SNT residuals before=${residualsBefore}`);
  if (pool.length === 0) {
    log("OCR: no vision provider configured -> clean no-op, not confirming. HONEST EMPTY.");
    report.ocr = { skipped: "no-vision-provider", residualsBefore };
    return;
  }
  const run = await tryInvoke<any>("run_company_ownership_ocr", { companyId: snt.id });
  if ((run as any).__err) { report.ocr = { error: (run as any).__err }; log(`OCR run error: ${(run as any).__err}`); return; }
  const proposals = (run as any).ocrProposals ?? [];
  log(`OCR SNT run: ${proposals.length} proposal(s)`);
  report.ocrProposals = proposals.length;
  if (proposals.length === 0) { report.ocr = { note: "no proposal produced (no table / provider empty)" }; return; }
  const p = proposals[0];
  log(`OCR proposal doc=${p.reportDocumentId} asOf=${p.asOf} holders=${p.holders?.length} provider=${p.providerId}`);
  // screenshot the proposal card best-effort (Review queue) is handled in screens test
  const confirmed = await invoke<any>("confirm_ownership_ocr_proposal", { companyId: snt.id, reportDocumentId: p.reportDocumentId });
  const remaining = confirmed?.ocrProposals?.length ?? 0;
  const holdersNow = confirmed?.holders?.length ?? 0;
  const residualsAfter = confirmed?.residuals?.length ?? 0;
  report.ocr = { confirmed: true, proposalsBefore: proposals.length, remaining, holdersNow, residualsBefore, residualsAfter };
  log(`OCR confirmed SNT: remaining=${remaining} holders=${holdersNow} residuals ${residualsBefore}->${residualsAfter}`);
  expect(remaining).toBeLessThan(proposals.length);
});

test("S5 — OLD-feature regression (backend reads)", async () => {
  test.setTimeout(240_000);
  const old: Record<string, unknown> = {};
  // Feed
  const feed = await tryInvoke<any[]>("list_feed_items");
  old.feedItems = Array.isArray(feed) ? feed.length : feed;
  // Fundamentals for 3 companies
  const fundamentals: any[] = [];
  for (const tk of ["ACP", "TXT", "KGH"]) {
    const c = byTicker(tk); if (!c) continue;
    const facts = await tryInvoke<any[]>("list_financial_facts", { input: { companyId: c.id } }).catch(() => null);
    const cov = await tryInvoke<any>("get_fundamentals_coverage", { companyId: c.id });
    fundamentals.push({ tk, facts: Array.isArray(facts) ? facts.length : (facts as any)?.__err ?? facts, coverage: (cov as any)?.__err ? (cov as any).__err : "ok" });
  }
  old.fundamentals = fundamentals;
  // Claims, journal, docs, coverage for ACP
  const acp = byTicker("ACP")!;
  old.claims = await tryInvoke<any[]>("list_management_claims", { input: { companyId: acp.id } }).then((r) => Array.isArray(r) ? r.length : r);
  old.decisionJournal = await tryInvoke<any[]>("list_decision_entries", {}).then((r) => Array.isArray(r) ? r.length : r);
  const docsView = await tryInvoke<any>("get_report_documents_view", { companyId: acp.id });
  old.reportDocs = (docsView as any)?.__err ?? (docsView as any)?.documents?.length ?? (docsView as any)?.groups?.length ?? "shape?";
  // Alerts: rules + fired events
  old.alertRules = await tryInvoke<any[]>("list_alert_rules").then((r) => Array.isArray(r) ? r.length : r);
  old.attentionEvents = await tryInvoke<any[]>("list_attention_events").then((r) => Array.isArray(r) ? r.length : r);
  // Briefing
  const briefing = await tryInvoke<any>("get_latest_morning_briefing");
  old.latestBriefing = (briefing as any)?.__err ? (briefing as any).__err : (briefing ? "present" : "none");
  // Calendar / events
  old.reportSeason = await tryInvoke<any[]>("list_report_season").then((r) => Array.isArray(r) ? r.length : r);
  old.companyEvents = await tryInvoke<any[]>("list_company_events", { input: { companyId: acp.id } }).then((r) => Array.isArray(r) ? r.length : r);
  // Shorts
  const shortsC = byTicker("CDR") ?? acp;
  const shorts = await tryInvoke<any>("list_short_positions", { input: { companyId: shortsC.id } });
  old.shorts = (shorts as any)?.__err ?? `${shortsC.ticker}: ${JSON.stringify(shorts).slice(0, 120)}`;
  // Diff candidates
  const diff = await tryInvoke<any>("list_report_diff_candidates", { input: { companyId: acp.id } });
  old.diffCandidates = (diff as any)?.__err ?? (diff as any)?.candidates?.length ?? (Array.isArray(diff) ? diff.length : "shape?");
  // Quality evaluate on 2 companies (reuse framework)
  const fws = await tryInvoke<any[]>("list_quality_frameworks");
  const fwId = Array.isArray(fws) && fws[0]?.id;
  const q: any[] = [];
  if (fwId) for (const tk of ["ACP", "TXT"]) {
    const c = byTicker(tk)!;
    const ev = await tryInvoke<any>("evaluate_framework", { input: { frameworkId: fwId, companyId: c.id } });
    q.push({ tk, ok: !(ev as any)?.__err, crit: (ev as any)?.criteria?.length ?? (ev as any)?.results?.length, err: (ev as any)?.__err });
  }
  old.qualityEval = q;
  report.old = old;
  log(`OLD FEATURES: ${JSON.stringify(old, null, 2)}`);
});

test("S6 — backfill progress across the 33 uncovered companies", async () => {
  test.setTimeout(180_000);
  const rows: any[] = [];
  let withDocs = 0, withPeriodic = 0, withFacts = 0;
  for (const c of companies) {
    const cov = await tryInvoke<any>("get_fundamentals_coverage", { companyId: c.id });
    const docsView = await tryInvoke<any>("get_report_documents_view", { companyId: c.id });
    const docs = (docsView as any)?.documents?.length ?? (docsView as any)?.groups?.reduce?.((a: number, g: any) => a + (g.documents?.length ?? 0), 0) ?? 0;
    const periodic = (docsView as any)?.documents?.filter?.((d: any) => /periodic|annual|quarter|semi/i.test(d.reportType ?? d.kind ?? ""))?.length ?? undefined;
    const facts = await tryInvoke<any[]>("list_financial_facts", { input: { companyId: c.id } });
    const factCount = Array.isArray(facts) ? facts.length : 0;
    if (docs > 0) withDocs++;
    if (factCount > 0) withFacts++;
    rows.push({ tk: c.ticker, docs, periodic, facts: factCount });
  }
  report.backfill = { withDocs, withFacts, total: companies.length, rows };
  log(`BACKFILL: companies withDocs=${withDocs} withFacts=${withFacts} of ${companies.length}`);
  rows.filter((r) => r.docs > 0 || r.facts > 0).forEach((r) => log(`  ${r.tk}: docs=${r.docs} facts=${r.facts}`));
});
