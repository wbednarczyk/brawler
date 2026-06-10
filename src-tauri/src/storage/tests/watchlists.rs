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
fn watchlist_memberships_accept_future_exchange_companies() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let company = state
        .create_company(NewCompany {
            exchange: "XETRA".to_owned(),
            ticker: "SAP".to_owned(),
            display_name: "SAP SE".to_owned(),
            isin: Some("DE0007164600".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("future exchange company should be created");
    let watchlist = state
        .create_watchlist(NewWatchlist {
            name: "Europe".to_owned(),
            description: None,
        })
        .expect("watchlist should be created");

    state
        .add_company_to_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("future exchange company should be assigned");

    let memberships = state
        .list_watchlist_memberships()
        .expect("memberships should list");
    let watchlists = state.list_watchlists().expect("watchlists should list");

    assert_eq!(memberships.len(), 1);
    assert_eq!(memberships[0].company_id, company.id);
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
fn renames_watchlist_without_changing_id_or_memberships() {
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
            watchlist_id: watchlist.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("company should be assigned");

    let renamed = state
        .rename_watchlist(WatchlistUpdate {
            id: watchlist.id.clone(),
            name: "Long-term GPW".to_owned(),
            description: Some("Long-term group".to_owned()),
        })
        .expect("watchlist should be renamed");

    assert_eq!(renamed.id, watchlist.id);
    assert_eq!(renamed.name, "Long-term GPW");
    assert_eq!(renamed.description.as_deref(), Some("Long-term group"));
    assert_eq!(renamed.company_count, 1);

    let memberships = state
        .list_watchlist_memberships()
        .expect("memberships should list");

    assert_eq!(memberships.len(), 1);
    assert_eq!(memberships[0].watchlist_id, watchlist.id);
    assert_eq!(memberships[0].watchlist_name, "Long-term GPW");
    assert_eq!(memberships[0].company_id, company.id);
}

#[test]
fn deletes_watchlist_and_keeps_member_companies() {
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
        .delete_watchlist(&watchlist.id)
        .expect("watchlist should be deleted");

    let watchlists = state.list_watchlists().expect("watchlists should list");
    let memberships = state
        .list_watchlist_memberships()
        .expect("memberships should list");
    let companies = state.list_companies().expect("companies should list");

    assert!(watchlists.is_empty());
    assert!(memberships.is_empty());
    assert_eq!(companies.len(), 1);
    assert_eq!(companies[0].id, company.id);
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
