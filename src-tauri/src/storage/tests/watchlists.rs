use super::*;

#[test]
fn creates_watchlist_and_assigns_company() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created");

    let watchlist = state
        .create_watchlist(NewWatchlist {
            name: "Main GPW".to_owned(),
            description: Some("Primary Polish watchlist".to_owned()),
        })
        .expect("watchlist should be created");

    state
        .add_company_to_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id,
            company_id: company.id,
        })
        .expect("company should be assigned");

    let watchlists = state.list_watchlists().expect("watchlists should list");

    assert_eq!(watchlists.len(), 1);
    assert_eq!(watchlists[0].name, "Main GPW");
    assert_eq!(watchlists[0].company_count, 1);
}

#[test]
fn lists_watchlist_memberships() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created");

    let watchlist = state
        .create_watchlist(NewWatchlist {
            name: "Main GPW".to_owned(),
            description: None,
        })
        .expect("watchlist should be created");

    state
        .add_company_to_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("company should be assigned");

    let memberships = state
        .list_watchlist_memberships()
        .expect("memberships should list");

    assert_eq!(memberships.len(), 1);
    assert_eq!(memberships[0].watchlist_id, watchlist.id);
    assert_eq!(memberships[0].watchlist_name, "Main GPW");
    assert_eq!(memberships[0].company_id, company.id);
}

#[test]
fn removes_company_from_watchlist() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created");

    let watchlist = state
        .create_watchlist(NewWatchlist {
            name: "Main GPW".to_owned(),
            description: None,
        })
        .expect("watchlist should be created");

    state
        .add_company_to_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("company should be assigned");

    state
        .remove_company_from_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id,
            company_id: company.id,
        })
        .expect("company should be removed");

    let watchlists = state.list_watchlists().expect("watchlists should list");

    assert_eq!(watchlists[0].company_count, 0);
}
