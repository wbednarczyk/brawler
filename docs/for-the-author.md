# Brawler dla autora (human-only)

> Dokument dla człowieka, nie dla agenta: zwięzły obraz tego, z czego składa się aplikacja i co potrafi DZIŚ. Aktualizowany przy każdym release (krok w skillu `brawler-release`). Wersja interaktywna: prywatny Artifact „Brawler — mapa systemu”. Normy i szczegóły są gdzie indziej (contracts, data-model, ADR-y) — tu ma być czytelnie, nie wyczerpująco.

**Stan: v0.53** · 10 trybów/ekranów · ~150 typowanych komend · 14 domen silnika (+ dane rynkowe) · 76 migracji · 4 narzędzia MCP · ~1900 testów w jednej bramce.

## Pięć warstw (przekrój od góry)

**1 · Co widzisz** — tryby: Dziś/Puls (poranny triage + powiadomienia autopilota), Spółki→Kokpit badawczy (panele per spółka: fundamenty, pokrycie, dokumenty, jakość, notatnik, dziennik…), Inbox/Kanał, Research, Listy, Transkrypcje, Źródła, Ustawienia, Diagnostyka. Paleta `Ctrl+K` otwiera panele i komendy. Nowe w v0.53: panel **Podstawowe informacje** (nazwa/ticker/ISIN/sektor/liczba akcji, tylko do odczytu — edycja pod jednym przyciskiem), **Kontekst cenowy** na czele panelu Fundamenty (kurs + zmiana, zakres 52 tyg., kapitalizacja, wskaźniki poziomu 0: C/Z, C/WK, EV/EBITDA, stopa dywidendy, FCF yield, percentyl w zakresie 52 tyg., **wykres świecowy** z okrągłą skalą), oraz **sektory** klasyfikowane automatycznie z rejestru z ręcznym nadpisaniem (podpowiedzi type-to-filter).

**2 · Rozmowa UI↔silnik** — każdy klik woła nazwaną, typowaną komendę (kontrakty w `docs/contracts.md`); nowe komendy zwracają kopertę błędów `kod+wiadomość`; bliźniaczy mock silnika pozwala testować UI, a korpus „fidelity” pilnuje, żeby mock nie kłamał.

**3 · Silnik (Rust), domeny** — feed+sygnały ESPI · fundamenty (fakty z okresami i walidacją; ekstrakcja warstwowa ESEF→xHTML→PDF→świadek-agregator→AI z budżetem) · **dane rynkowe** (dzienne notowania EOD z Yahoo do `daily_quotes`, backfill od debiutu + samonaprawa historii; sektory z rejestru; wskaźniki poziomu 0 jako kanoniczne metryki pochodne z kaskadą „licz z czego się da”) · pokrycie+sweep po backfillu · raporty+diff raport-do-raportu · claimy zarządu z kolejką weryfikacji · jakość (frameworki DSL + oceny AI ze scorecardem) · **osąd** (dziennik append-only + oczekiwania zamrażane po potwierdzeniu faktów) · autopilot (wykryj→pobierz→wyekstrahuj→diff→cross-ref→jedno powiadomienie; drabina zaufania) · research+AI teksty (briefy/digesty; zawsze decision-support, nigdy kup/sprzedaj) · kalendarz zdarzeń · transkrypcje · trwała kolejka jobów z pasmami · **serwer MCP** (drugi „wjazd” do tych samych domen, tylko odczyt).

**4 · Fundament** — jedna baza SQLite (migracje append-only, FTS5), rotacyjne backupy + snapshot przed migracją, sekrety wyłącznie w keychainie systemowym, ustawienia z tolerancyjnym odczytem (stara baza zawsze się otworzy).

**5 · Świat zewnętrzny** — do środka: Bankier (ESPI/EBI+giełda), RSS, agregatory jako świadek; na żądanie: providerzy AI (Gemini/Anthropic/OpenAI/Mistral/kompatybilni, klucze z keychaina); **do Ciebie**: klienci MCP (Claude i inni) przez `127.0.0.1`+token — dossier, szukanie w researchu, claimy do weryfikacji, ocena jakości.

## Strażnicy (w poprzek wszystkiego)

Jedno `make check` przed każdym commitem, twardo: testy Rust+UI+przeglądarka, journeys z budżetami kliknięć, docs-drift (dokumentacja≠kod→stop), gate-integrity (bramek nie da się po cichu wyłączyć), tłumaczenia PL+EN, martwy kod (knip+strażnik API), disk-guard, baseliny wizualne, ratchet pokrycia (tylko w górę). Filozofia: nie ufasz modelowi ani sobie — ufasz bramkom.

## Gdzie co znaleźć (jako użytkownik)

- Zapis decyzji: kokpit spółki → `+ Dodaj panel` → **Dziennik decyzji** (globalnie: paleta → „Dziennik (wszystkie spółki)”).
- Oczekiwania przed raportem: panel **Sezon raportów** → karta spółki → **Zapisz oczekiwania**; przegląd „oczekiwane vs fakty” pojawia się po potwierdzeniu danych.
- MCP: Ustawienia → **Serwer MCP** (włącznik, port, token pokazany raz, gotowe snippety dla Claude).
- Instrukcje użytkowe per funkcja: katalog `wiki/`.
