//! Parser unit tests over **synthetic** cover-note bodies that mirror the
//! ground-truth structural classes (single PDMR / closely-associated-with-anchor
//! / multi-unit enumerated / conjunction / bilingual truncation / attachment
//! descriptions / warrants). Bodies are hand-written to be structurally
//! equivalent to the real filings — never copied verbatim from `private/`.

use super::*;

fn clean_units(parse: &InsiderNotificationParse) -> Vec<ParsedInsiderUnit> {
    match parse {
        InsiderNotificationParse::Units(units) => units
            .iter()
            .filter_map(|u| match u {
                InsiderUnit::Clean(p) => Some(p.clone()),
                InsiderUnit::Ambiguous { .. } => None,
            })
            .collect(),
        InsiderNotificationParse::NotFound => Vec::new(),
    }
}

fn units_of(title: &str, body: &str) -> Vec<ParsedInsiderUnit> {
    clean_units(&parse_insider_notification(title, body))
}

// ---- Name normalization (genitive / instrumental → nominative) ----

#[test]
fn normalizes_genitive_and_instrumental_names() {
    // Masculine genitive: strip -a.
    assert_eq!(normalize_person("Andrzeja Oślizło"), "ANDRZEJ OŚLIZŁO");
    assert_eq!(normalize_person("Piotra Kulessa"), "PIOTR KULESSA");
    // Feminine genitive: -y → -a.
    assert_eq!(normalize_person("Martyny Kulessa"), "MARTYNA KULESSA");
    // Fleeting-e restoration.
    assert_eq!(normalize_person("Pawła Nowak"), "PAWEŁ NOWAK");
    // Instrumental (enumerated PDMR anchors).
    assert_eq!(normalize_person("Robertem Stasikiem"), "ROBERT STASIK");
    assert_eq!(normalize_person("Adamem Lewkowiczem"), "ADAM LEWKOWICZ");
    assert_eq!(normalize_person("Krzysztofem Szyszką"), "KRZYSZTOF SZYSZKA");
    assert_eq!(
        normalize_person("Mikołajem Podgórskim"),
        "MIKOŁAJ PODGÓRSKI"
    );
    // Surname ending -ski stays (never corrupted to -ska).
    assert_eq!(
        normalize_person("Jędrzejem Kowalewski"),
        "JĘDRZEJ KOWALEWSKI"
    );
    // Zięby → Zięba.
    assert_eq!(normalize_person("Michała Zięby"), "MICHAŁ ZIĘBA");
}

#[test]
fn keeps_entity_names_verbatim() {
    // Entity kept verbatim (trailing sentence punctuation is normalized away).
    assert_eq!(
        normalize_person("Książek Holding Sp. z o.o."),
        "KSIĄŻEK HOLDING SP. Z O.O"
    );
    assert_eq!(
        normalize_person("MR3M Fundacja Rodzinna"),
        "MR3M FUNDACJA RODZINNA"
    );
}

// ---- Single-PDMR management filing (KRU/MDV shape) ----

#[test]
fn single_management_pdmr_buy_shares() {
    let title = "Informacja o transakcjach uzyskana w trybie art. 19 MAR";
    let body = "Spis treści:1. RAPORT BIEŻĄCYKOMISJA NADZORU FINANSOWEGO\
        Treść raportu:Zarząd Przykład S.A. informuje o otrzymaniu w dniu dzisiejszym \
        od Jana Testowego, Prezesa Zarządu Spółki, powiadomienia w trybie art. 19 MAR \
        o transakcji nabycia akcji Emitenta.\
        ZałącznikiPlikOpisPowiadomienie.pdfMESSAGE (ENGLISH VERSION)the board of Przyklad informs";
    let parse = parse_insider_notification(title, body);
    let units = clean_units(&parse);
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].person_normalized, "JAN TESTOWY");
    assert_eq!(units[0].role, Some(InsiderRole::Management));
    assert_eq!(units[0].direction, Some(Direction::Buy));
    assert_eq!(units[0].instrument, Some(Instrument::Shares));
    // Bilingual half never reaches the parser (truncation).
    assert!(!units[0].person_raw.to_lowercase().contains("board"));
}

// ---- Direction absent → NULL (KRU "powiadomienie o transakcjach" shape) ----

#[test]
fn direction_absent_stays_null() {
    let title = "Powiadomienie o transakcji osoby pełniącej obowiązki zarządcze";
    let body = "Treść raportu:Zarząd Przykład S.A. informuje, że otrzymał od Jana Testowego, \
        Członka Zarządu, powiadomienie o transakcjach na akcjach Spółki. \
        Pełna treść powiadomienia znajduje się w załączniku.";
    let units = units_of(title, body);
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].direction, None, "no nabycie/zbycie word → NULL");
    assert_eq!(units[0].role, Some(InsiderRole::Management));
}

// ---- Bare PDMR, no board qualifier → role NULL (never guessed) ----

#[test]
fn bare_pdmr_role_null() {
    let title = "Informacja o transakcji w trybie art. 19 MAR";
    let body = "Treść raportu:Zarząd Przykład S.A. informuje o otrzymaniu od Anny Przykład \
        jako osoby pełniącej obowiązki zarządcze powiadomienia o transakcji nabycia akcji.";
    let units = units_of(title, body);
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].person_normalized, "ANNA PRZYKŁAD");
    assert_eq!(units[0].role, None, "no board qualifier → NULL");
    assert_eq!(units[0].direction, Some(Direction::Buy));
}

// ---- Closely-associated warrants (DVL shape) ----

#[test]
fn closely_associated_warrants_via_bare_pdmr() {
    let title = "Informacja o transakcji nabycia przez osobę pełniącą obowiązki zarządcze \
        warrantów subskrypcyjnych serii A Emitenta";
    let body = "Treść raportu:Zarząd Przykład S.A. informuje o otrzymaniu w dniu dzisiejszym \
        od Andrzeja Testowego jako osoby pełniącej obowiązki zarządcze, powiadomienia w trybie \
        art. 19 ust. 1 rozporządzenia MAR o transakcji nabycia warrantów subskrypcyjnych serii A.";
    let units = units_of(title, body);
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].person_normalized, "ANDRZEJ TESTOWY");
    assert_eq!(units[0].instrument, Some(Instrument::SubscriptionWarrants));
    assert_eq!(units[0].direction, Some(Direction::Buy));
}

// ---- Two-party conjunction: supervisory + closely-associated (GKI shape) ----

#[test]
fn conjunction_two_parties_supervisory_plus_associate() {
    let title =
        "Zawiadomienie Członka Rady Nadzorczej i osoby blisko związanej o nabyciu akcji Emitenta";
    let body = "Treść raportu:Przykład S.A. informuje, iż otrzymała w trybie art. 19 ust. 1 \
        Rozporządzenia MAR od Pana Piotra Testowego Członka Rady Nadzorczej Spółki i osoby \
        blisko z nim związanej p. Martyny Testowej, zawiadomienia o nabyciu akcji Spółki.";
    let units = units_of(title, body);
    assert_eq!(units.len(), 2);
    assert_eq!(units[0].person_normalized, "PIOTR TESTOWY");
    assert_eq!(units[0].role, Some(InsiderRole::Supervisory));
    assert_eq!(units[1].person_normalized, "MARTYNA TESTOWA");
    assert_eq!(units[1].role, Some(InsiderRole::CloselyAssociated));
    assert_eq!(
        units[1].related_pdmr_normalized.as_deref(),
        Some("PIOTR TESTOWY")
    );
    assert!(units.iter().all(|u| u.direction == Some(Direction::Buy)));
}

// ---- Four enumerated units, closely-associated with named PDMRs (VRC shape) ----

#[test]
fn enumerated_four_units_with_pdmr_anchors() {
    let title = "Zawiadomienie o transakcjach nabycia w trybie art. 19 MAR";
    let body = "Treść raportu:Zarząd Przykład S.A. informuje, że otrzymał powiadomienia o \
        transakcjach wykonanych na akcjach Spółki od \
        [i] Alfa S.A. tj. podmiotu blisko związanego z osobą pełniącą obowiązki zarządcze w Spółce - Robertem Testowym, Przewodniczącym Rady Nadzorczej Spółki, \
        [ii] Beta Fundacja Rodzinna tj. podmiotu blisko związanego z osobą pełniącą obowiązki zarządcze w Spółce – Adamem Testowym, Wiceprezesem Zarządu Spółki, \
        [iii] Gamma Fundacja Rodzinna tj. podmiotu blisko związanego z osobą pełniącą obowiązki zarządcze w Spółce – Krzysztofem Testowskim, Prezesem Zarządu Spółki i \
        [iv] Joanny Testowej tj. osoby blisko związanej z osobą pełniącą obowiązki zarządcze w Spółce – Krzysztofem Testowskim, Prezesem Zarządu Spółki.";
    let units = units_of(title, body);
    assert_eq!(units.len(), 4);
    assert!(units
        .iter()
        .all(|u| u.role == Some(InsiderRole::CloselyAssociated)));
    assert!(units.iter().all(|u| u.direction == Some(Direction::Buy)));
    assert_eq!(units[0].related_pdmr_role, Some(InsiderRole::Supervisory));
    assert_eq!(units[1].related_pdmr_role, Some(InsiderRole::Management));
}

// ---- Attachment-description units + transfer/sell direction (SCW shape) ----

#[test]
fn attachment_descriptions_yield_units() {
    let title = "Powiadomienia o transakcji osoby pełniącej obowiązki oraz osoby blisko związanej otrzymane w trybie art. 19 MAR";
    let body = "Treść raportu:Zarząd Przykład S.A. informuje, że wpłynęły powiadomienia w trybie \
        art. 19 MAR od Michała Testowego jako Członka Zarządu oraz dwóch podmiotów blisko \
        związanych (fundacje rodzinne) z Mikołajem Testowskim (Członek Zarządu) oraz Jędrzejem \
        Testowski (Prezes Zarządu). Transakcje dot. sprzedaży akcji Spółki w ramach ABB.\
        ZałącznikiPlikOpis\
        Powiadomienie JKFR.pdfPowiadomienie Jędrzej Testowski Fundacja Rodzinna\
        Powiadomienie PFR.pdfPowiadomienie Testowscy Fundacja Rodzinna w organizacji\
        Powiadomienie MZ.pdfPowiadomienie Michał Testowy\
        MESSAGE (ENGLISH VERSION)";
    let units = units_of(title, body);
    assert_eq!(units.len(), 3);
    assert!(units.iter().all(|u| u.direction == Some(Direction::Sell)));
    // The entity descriptions classify as closely-associated; the natural person
    // as management.
    let michal = units
        .iter()
        .find(|u| u.person_raw.contains("Michał Testowy"));
    assert!(michal.is_some());
    assert_eq!(michal.unwrap().role, Some(InsiderRole::Management));
    assert!(units
        .iter()
        .any(|u| u.role == Some(InsiderRole::CloselyAssociated)));
}

// ---- Transfer (przeniesienie) → direction Other (SCW przeniesienie shape) ----

#[test]
fn transfer_direction_is_other() {
    let title = "Powiadomienie o transakcji w trybie art. 19 MAR";
    let body = "Treść raportu:Zarząd Przykład S.A. informuje o otrzymaniu od Jana Testowego, \
        Członka Zarządu, powiadomienia o przeniesieniu akcji Spółki.";
    let units = units_of(title, body);
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].direction, Some(Direction::Other));
}

// ---- Inline figures: operative-sentence volume/price/currency/date (MDV shape) ----

#[test]
fn inline_figures_mdv_operative_sentence() {
    // Cover note restates the full transaction inline: volume, average price with
    // currency, and the Polish long-form transaction date; a post-tx holdings
    // figure ("74 500 akcji") follows and must NOT be taken as the volume.
    let title = "Informacja o transakcjach na akcjach Spółki uzyskana w trybie art. 19 MAR";
    let body = "Treść raportu:Zarząd Przykład S.A. z siedzibą w Miastowie (\"Emitent\") \
        informuje o otrzymaniu w dniu 6 lipca 2026 roku powiadomienia od Pana Jana Testowego \
        – Wiceprezesa Zarządu Emitenta, w trybie art. 19 ust. 1 rozporządzenia MAR o \
        transakcjach na akcjach Emitenta, o nabyciu przez niego w dniu 6 lipca 2026 roku na \
        rynku regulowanym GPW, 4500 akcji zwykłych na okaziciela, po średniej cenie 102,56 PLN \
        za jedną akcję. Na dzień przekazania raportu Pan Jan Testowy posiada 74 500 akcji \
        Emitenta.ZałącznikiPlikOpisZawiadomienie.pdfZawiadomienie z art. 19 MAR\
        MESSAGE (ENGLISH VERSION)";
    let units = units_of(title, body);
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].person_normalized, "JAN TESTOWY");
    assert_eq!(units[0].role, Some(InsiderRole::Management));
    assert_eq!(units[0].direction, Some(Direction::Buy));
    assert_eq!(units[0].volume.as_deref(), Some("4500"));
    assert_eq!(units[0].price.as_deref(), Some("102.56"));
    assert_eq!(units[0].currency.as_deref(), Some("PLN"));
    assert_eq!(units[0].tx_date.as_deref(), Some("2026-07-06"));
}

// ---- Inline figures: dot-grouped volume + long-form date, price PDF-only (CMP shape) ----

#[test]
fn inline_figures_cmp_volume_and_date_first_transaction() {
    // A closely-associated vehicle with two disposals in one note: a completed
    // sale (275.000 on 3 lipca) then a future-tense conditional one (40.000 on
    // 7 lipca). The single cover-note unit carries the FIRST (completed) figures;
    // price is absent (PDF-only) and stays NULL.
    let title = "Zawiadomienie o zawarciu transakcji przez osobę blisko związaną";
    let body = "Treść raportu:Przykład S.A. z siedzibą w Miastowie informuje, że otrzymała \
        powiadomienie o zbyciu akcji Przykład S.A. złożone w trybie art. 19 ust. 1 \
        rozporządzenia MAR, przez TT Inwestycje Alternatywna Spółka Inwestycyjna sp. z o.o. z \
        siedzibą w Miastowie, tj. osobę blisko związaną z osobą pełniącą obowiązki zarządcze w \
        spółce: Panem Robertem Testowskim, Prezesem Zarządu. Na podstawie transakcji na rynku \
        regulowanym, zawartej w dniu 3 lipca 2026 r. TT Inwestycje Alternatywna Spółka \
        Inwestycyjna sp. z o.o. zbyła 275.000 akcji Przykład S.A., stanowiących 1,34% kapitału \
        zakładowego. Ponadto na podstawie transakcji zawartej poza rynkiem regulowanym w dniu \
        7 lipca 2026 r. TT Inwestycje Alternatywna Spółka Inwestycyjna sp. z o.o., po \
        rozliczeniu tej transakcji, zbędzie 40.000 akcji Przykład S.A.MESSAGE (ENGLISH VERSION)";
    let units = units_of(title, body);
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].role, Some(InsiderRole::CloselyAssociated));
    assert_eq!(units[0].direction, Some(Direction::Sell));
    assert_eq!(units[0].volume.as_deref(), Some("275000"));
    assert_eq!(units[0].price, None, "price is PDF-only here → NULL");
    assert_eq!(units[0].currency, None);
    assert_eq!(units[0].tx_date.as_deref(), Some("2026-07-03"));
}

// ---- Inline figures: ESAP metadata date block, no operative figures (DVL shape) ----

#[test]
fn inline_figures_dvl_esap_date_block() {
    // Warrant acquisition: the operative sentence carries NO volume/price (PDF),
    // but the ESAP metadata block restates the ISO transaction date. The receipt
    // phrasing "w dniu dzisiejszym tj. 15.07.2026" must not be mistaken for it —
    // the date comes from the ESAP block, and volume/price stay NULL.
    let title = "Informacja o transakcji nabycia przez osobę pełniącą obowiązki zarządcze \
        warrantów subskrypcyjnych serii A Emitenta";
    let body = "Treść raportu:Zarząd Przykład S.A. (\"Emitent\") informuje o otrzymaniu w dniu \
        dzisiejszym tj. 15.07.2026 r. od Jana Testowego jako osoby pełniącej obowiązki \
        zarządcze, powiadomienia w trybie art. 19 ust. 1 rozporządzenia MAR o transakcji \
        nabycia warrantów subskrypcyjnych serii A Emitenta. Pełna treść powiadomienia znajduje \
        się w załączniku.ZałącznikiPlikOpisPowiadomienie.pdfPowiadomienie art.19Ramy prawne\
        Rodzaj informacji przekazanych przez podmiotTRANSDTransakcja dokonywana przez osoby \
        pełniące obowiązki zarządczeData lub początek okresu, której lub którego dotyczą \
        informacje2026-07-15Data lub koniec okresu, której lub którego dotyczą informacje\
        2026-07-15MESSAGE (ENGLISH VERSION)";
    let units = units_of(title, body);
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].person_normalized, "JAN TESTOWY");
    assert_eq!(units[0].instrument, Some(Instrument::SubscriptionWarrants));
    assert_eq!(units[0].direction, Some(Direction::Buy));
    assert_eq!(units[0].volume, None, "warrant count is PDF-only → NULL");
    assert_eq!(units[0].price, None);
    assert_eq!(units[0].tx_date.as_deref(), Some("2026-07-15"));
}

// ---- Inline figures: receipt date without an inline volume → tx_date NULL ----

#[test]
fn inline_figures_receipt_date_without_volume_stays_null() {
    // The cover note dates only the RECEIPT of the notification ("otrzymał w dniu
    // 22 czerwca 2026 …") and carries no per-transaction volume. That date is not
    // a transaction date — with no inline volume to anchor it, tx_date stays NULL.
    let title = "Powiadomienie o transakcjach osoby pełniącej obowiązki zarządcze";
    let body = "Treść raportu:Zarząd Przykład S.A. informuje o otrzymaniu w dniu 22 czerwca \
        2026 roku od Jana Testowego, Członka Zarządu, powiadomienia o transakcjach na akcjach \
        Spółki. Pełna treść powiadomienia znajduje się w załączniku.";
    let units = units_of(title, body);
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].volume, None);
    assert_eq!(units[0].price, None);
    assert_eq!(
        units[0].tx_date, None,
        "receipt date must not become tx_date"
    );
}

// ---- Not an insider notification → NotFound ----

#[test]
fn unrelated_body_is_not_found() {
    let title = "Rekomendacja Zarządu w sprawie wypłaty dywidendy";
    let body = "Treść raportu:Zarząd rekomenduje wypłatę dywidendy w wysokości 1,00 zł na akcję.";
    let units = units_of(title, body);
    assert!(units.is_empty());
}

#[test]
fn empty_body_is_not_found() {
    assert_eq!(
        parse_insider_notification("t", ""),
        InsiderNotificationParse::NotFound
    );
}

// ---- Idempotence: re-parse produces identical units ----

#[test]
fn reparse_is_stable() {
    let title = "Informacja o transakcjach uzyskana w trybie art. 19 MAR";
    let body = "Treść raportu:Zarząd Przykład S.A. informuje o otrzymaniu od Jana Testowego, \
        Prezesa Zarządu, powiadomienia o transakcji nabycia akcji.";
    let a = parse_insider_notification(title, body);
    let b = parse_insider_notification(title, body);
    assert_eq!(a, b);
}
