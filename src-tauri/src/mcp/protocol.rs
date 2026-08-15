//! Pure JSON-RPC 2.0 dispatcher for the MCP subset (ADR 0078 decision 3).
//!
//! Implements exactly: `initialize` (protocol-version negotiation against the
//! pinned [`SUPPORTED_PROTOCOL_VERSION`]), `notifications/initialized`, `ping`,
//! `tools/list`, `tools/call`, plus the JSON-RPC error responses −32700 (parse
//! error), −32600 (invalid request, including batch arrays — removed in the
//! 2025-06-18 MCP revision), −32601 (method not found), −32602 (invalid
//! params) and −32603 (internal error). Notifications never produce a
//! response. Pure over `serde_json::Value` — fully testable without HTTP.

use serde_json::{json, Map, Value};

use super::registry;
use super::tools::{ToolCallError, ToolOutcome};
use crate::app_state::AppState;

/// The pinned MCP protocol revision this server implements (ADR 0078
/// decision 3). Batch JSON-RPC requests were removed in this revision, which
/// is why the dispatcher rejects arrays with −32600.
pub const SUPPORTED_PROTOCOL_VERSION: &str = "2025-06-18";

/// Dispatch one raw JSON-RPC message under the caller's authenticated scope
/// (ADR 0099 dec. 2 — resolved by the server from the matched bearer digest).
/// Returns `Some(response_json)` for requests (success or error envelope) and
/// `None` for notifications, which must not be answered (the HTTP layer maps
/// `None` to `202 Accepted`).
pub fn dispatch(state: &AppState, scope: registry::McpScope, raw: &str) -> Option<String> {
    let message: Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(error) => {
            return Some(error_response(
                Value::Null,
                -32700,
                &format!("parse error: {error}"),
            ))
        }
    };
    if message.is_array() {
        return Some(error_response(
            Value::Null,
            -32600,
            "batch requests are not supported (removed in the 2025-06-18 MCP revision)",
        ));
    }
    let Some(object) = message.as_object() else {
        return Some(error_response(
            Value::Null,
            -32600,
            "a JSON-RPC message must be an object",
        ));
    };
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Some(error_response(
            id_of(object),
            -32600,
            "jsonrpc must be \"2.0\"",
        ));
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return Some(error_response(
            id_of(object),
            -32600,
            "method must be a string",
        ));
    };
    let id = match object.get("id") {
        // No id ⇒ notification. `notifications/initialized` is accepted; every
        // notification — known or not — is answered with silence (JSON-RPC 2.0).
        None => return None,
        Some(id @ (Value::String(_) | Value::Number(_) | Value::Null)) => id.clone(),
        Some(_) => {
            return Some(error_response(
                Value::Null,
                -32600,
                "id must be a string, a number, or null",
            ))
        }
    };
    let params = object.get("params").cloned().unwrap_or(Value::Null);

    Some(match method {
        "initialize" => handle_initialize(id, &params),
        "ping" => success_response(id, json!({})),
        "tools/list" => success_response(id, json!({ "tools": registry::descriptors(scope) })),
        "tools/call" => handle_tools_call(state, scope, id, &params),
        other => error_response(id, -32601, &format!("method not found: {other}")),
    })
}

/// `initialize`: negotiate the protocol revision (MCP 2025-06-18 lifecycle —
/// echo a supported requested revision, otherwise answer with the latest one
/// the server supports) and advertise the tools capability + server identity.
fn handle_initialize(id: Value, params: &Value) -> String {
    let Some(requested) = params.get("protocolVersion").and_then(Value::as_str) else {
        return error_response(id, -32602, "params.protocolVersion (string) is required");
    };
    let negotiated = if requested == SUPPORTED_PROTOCOL_VERSION {
        requested
    } else {
        SUPPORTED_PROTOCOL_VERSION
    };
    success_response(
        id,
        json!({
            "protocolVersion": negotiated,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": "brawler",
                "title": "Brawler — investor research workspace",
                "version": env!("CARGO_PKG_VERSION"),
            },
        }),
    )
}

/// `tools/call`: strict param validation (−32602), then run the tool. Domain
/// failures become a tool result with `isError: true` carrying the ADR 0070
/// error envelope; unknown tools / bad arguments are protocol errors (−32602).
fn handle_tools_call(
    state: &AppState,
    scope: registry::McpScope,
    id: Value,
    params: &Value,
) -> String {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return error_response(id, -32602, "params.name (string) is required");
    };
    let arguments = match params.get("arguments") {
        None | Some(Value::Null) => Value::Object(Map::new()),
        Some(arguments @ Value::Object(_)) => arguments.clone(),
        Some(_) => return error_response(id, -32602, "params.arguments must be an object"),
    };
    match registry::call(state, scope, name, &arguments) {
        Ok(ToolOutcome::Success(payload)) => success_response(
            id,
            json!({
                "content": [{ "type": "text", "text": payload.to_string() }],
                "structuredContent": payload,
                "isError": false,
            }),
        ),
        Ok(ToolOutcome::Failure(error)) => match serde_json::to_value(&error) {
            Ok(envelope) => success_response(
                id,
                json!({
                    "content": [{ "type": "text", "text": envelope.to_string() }],
                    "isError": true,
                }),
            ),
            Err(error) => error_response(id, -32603, &format!("internal error: {error}")),
        },
        Err(ToolCallError::UnknownTool(tool)) => {
            error_response(id, -32602, &format!("unknown tool: {tool}"))
        }
        Err(ToolCallError::InvalidArguments(message)) => error_response(id, -32602, &message),
    }
}

fn success_response(id: Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn error_response(id: Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}

/// The response id for a malformed request: the message's id when it carries a
/// valid one, `null` otherwise (JSON-RPC 2.0 §5).
fn id_of(object: &Map<String, Value>) -> Value {
    match object.get("id") {
        Some(id @ (Value::String(_) | Value::Number(_) | Value::Null)) => id.clone(),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{open_in_memory_database, AppState};
    use proptest::prelude::*;
    use serde_json::{json, Value};
    use std::sync::OnceLock;

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    /// One migrated in-memory state shared across proptest cases (running the
    /// full migration corpus per generated input would dominate the test).
    fn shared_state() -> &'static AppState {
        static STATE: OnceLock<AppState> = OnceLock::new();
        STATE.get_or_init(state)
    }

    fn dispatch_value(state: &AppState, raw: &str) -> Value {
        let response =
            dispatch(state, registry::McpScope::Full, raw).expect("a response for a request");
        serde_json::from_str(&response).expect("response is valid JSON")
    }

    fn error_code(response: &Value) -> i64 {
        response["error"]["code"]
            .as_i64()
            .unwrap_or_else(|| panic!("no error code in {response}"))
    }

    #[test]
    fn initialize_negotiates_version_and_lists_tools_capability() {
        let s = state();
        // An unsupported requested revision is answered with the pinned one.
        let v = dispatch_value(
            &s,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.0.0"}}}"#,
        );
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 1);
        assert_eq!(v["result"]["protocolVersion"], SUPPORTED_PROTOCOL_VERSION);
        assert!(
            v["result"]["capabilities"]["tools"].is_object(),
            "tools capability must be advertised: {v}"
        );
        assert_eq!(v["result"]["serverInfo"]["name"], "brawler");

        // The supported revision round-trips unchanged.
        let raw = format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"initialize","params":{{"protocolVersion":"{SUPPORTED_PROTOCOL_VERSION}"}}}}"#
        );
        let v = dispatch_value(&s, &raw);
        assert_eq!(v["result"]["protocolVersion"], SUPPORTED_PROTOCOL_VERSION);
    }

    #[test]
    fn unknown_method_is_minus_32601() {
        let s = state();
        let v = dispatch_value(&s, r#"{"jsonrpc":"2.0","id":5,"method":"resources/list"}"#);
        assert_eq!(error_code(&v), -32601);
        assert_eq!(v["id"], 5);
    }

    #[test]
    fn malformed_json_is_minus_32700() {
        let s = state();
        let v = dispatch_value(&s, "{ this is not json");
        assert_eq!(error_code(&v), -32700);
        assert!(v["id"].is_null());
    }

    #[test]
    fn batch_request_rejected_minus_32600() {
        let s = state();
        let v = dispatch_value(&s, r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#);
        assert_eq!(
            error_code(&v),
            -32600,
            "batch arrays were removed in the 2025-06-18 revision"
        );
        // An empty batch is equally invalid.
        let v = dispatch_value(&s, "[]");
        assert_eq!(error_code(&v), -32600);
    }

    #[test]
    fn invalid_params_is_minus_32602() {
        let s = state();
        // initialize without the required protocolVersion.
        let v = dispatch_value(
            &s,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        );
        assert_eq!(error_code(&v), -32602);

        // tools/call with an unknown tool name.
        let v = dispatch_value(
            &s,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"no_such_tool","arguments":{}}}"#,
        );
        assert_eq!(error_code(&v), -32602);

        // tools/call with an unknown argument field (deny_unknown_fields).
        let v = dispatch_value(
            &s,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_company_dossier","arguments":{"company":"GPW:TST","bogus":1}}}"#,
        );
        assert_eq!(error_code(&v), -32602);

        // tools/call without a name.
        let v = dispatch_value(
            &s,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{}}"#,
        );
        assert_eq!(error_code(&v), -32602);
    }

    #[test]
    fn notification_produces_no_response() {
        let s = state();
        assert_eq!(
            dispatch(
                &s,
                registry::McpScope::Full,
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
            ),
            None
        );
        // Any id-less message is a notification and is never answered, even
        // when the method is unknown.
        assert_eq!(
            dispatch(
                &s,
                registry::McpScope::Full,
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled"}"#
            ),
            None
        );
    }

    #[test]
    fn ping_round_trips() {
        let s = state();
        let v = dispatch_value(&s, r#"{"jsonrpc":"2.0","id":"ping-1","method":"ping"}"#);
        assert_eq!(v["id"], "ping-1");
        assert_eq!(v["result"], json!({}));
    }

    // ---- Frozen tool contract (ADR 0078 G-1) --------------------------------

    /// The FULL `tools/list` response. Changing the tool set or any schema is a
    /// reviewed spec change — never regenerate this snapshot casually.
    #[test]
    fn tools_list_snapshot_is_the_frozen_contract() {
        let s = state();
        let v = dispatch_value(&s, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
        insta::assert_snapshot!(
            "tools_list_schema",
            serde_json::to_string_pretty(&v).expect("serializable")
        );
    }

    /// The SCOPED `tools/list` response — the acquisition credential's whole
    /// frozen surface (ADR 0099 dec. 3/7, #386): exactly the nine workflow
    /// tools, and the serialized response stays under the 16 KiB byte gate
    /// (regression coverage for the compact-surface promise, contracts.md
    /// § Budgets — the gate is coverage, not the enforcement).
    #[test]
    fn tools_list_scoped_snapshot_is_the_frozen_contract() {
        let s = state();
        let response = dispatch(
            &s,
            registry::McpScope::KpiAcquisition,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        )
        .expect("a response for a request");
        let v: Value = serde_json::from_str(&response).expect("response is valid JSON");
        assert!(
            response.len() <= 16 * 1024,
            "the scoped tools/list must stay ≤16 KiB, got {} bytes",
            response.len()
        );
        insta::assert_snapshot!(
            "tools_list_schema_acquisition",
            serde_json::to_string_pretty(&v).expect("serializable")
        );
    }

    /// `get_company_dossier` over a seeded in-memory database. Timestamps are
    /// redacted (SQLite CURRENT_TIMESTAMP); every id in the tree is
    /// deterministic (slug/hash derived), so the rest snapshots stably.
    #[test]
    fn dossier_output_snapshot() {
        let s = state();
        seed_dossier_company(&s);
        let v = dispatch_value(
            &s,
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"get_company_dossier","arguments":{"company":"GPW:TST"}}}"#,
        );
        // Redact SQLite CURRENT_TIMESTAMP values (the in-tree insta has no
        // `filters` feature; the regex crate is already a dependency).
        let pretty = serde_json::to_string_pretty(&v).expect("serializable");
        let redacted =
            regex::Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(\.\d+)?(Z|\+\d{2}:\d{2})?")
                .expect("valid redaction regex")
                .replace_all(&pretty, "[timestamp]")
                .into_owned();
        insta::assert_snapshot!("dossier_output", redacted);
    }

    /// Manual real-client harness (ADR 0078 decision 8's verification aid; not
    /// part of the gate). Seeds the sample dossier company and serves a live
    /// MCP endpoint so a real client (Claude Code `--transport http`, curl,
    /// the stdio adapter) can be pointed at it:
    ///   BRAWLER_MCP_HARNESS_PORT  (default 8399)
    ///   BRAWLER_MCP_HARNESS_TOKEN (default "harness-token")
    ///   BRAWLER_MCP_HARNESS_SECS  (default 90)
    /// Run: `cargo nextest run -E 'test(real_client_harness)' --run-ignored all`
    #[test]
    #[ignore = "manual harness: serves a live MCP endpoint for real-client verification"]
    fn real_client_harness() {
        let env_or = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        let port: u16 = env_or("BRAWLER_MCP_HARNESS_PORT", "8399")
            .parse()
            .expect("harness port");
        let token = env_or("BRAWLER_MCP_HARNESS_TOKEN", "harness-token");
        let secs: u64 = env_or("BRAWLER_MCP_HARNESS_SECS", "90")
            .parse()
            .expect("harness secs");

        let s = state();
        seed_dossier_company(&s);
        let handle = crate::mcp::server::McpServerHandle::start(s, &token, None, port)
            .expect("harness server binds");
        eprintln!("MCP harness READY on http://127.0.0.1:{port}/mcp for {secs}s");
        std::thread::sleep(std::time::Duration::from_secs(secs));
        drop(handle);
    }

    fn seed_dossier_company(s: &AppState) {
        use crate::storage::{
            CaptureReportDocumentInput, NewCompany, NewFinancialFact, NewFinancialPeriod,
        };

        let company = s
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "TST".to_owned(),
                display_name: "Test S.A.".to_owned(),
                isin: Some("PLTEST0000011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company");

        // A fetched canonical annual report (period parsed from the title).
        let doc = s
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/ssf-2025.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Skonsolidowany raport roczny 2025 SSF".to_owned()),
                attribution: None,
            })
            .expect("report document");
        s.mark_report_document_fetched(
            &doc.id,
            Some("reports/ssf-2025.pdf"),
            Some("application/pdf"),
            Some("hash"),
            Some(2048),
        )
        .expect("mark fetched");

        let period = s
            .create_financial_period(NewFinancialPeriod {
                company_id: company.id.clone(),
                fiscal_year: 2025,
                period_type: "FY".to_owned(),
                period_end_date: Some("2025-12-31".to_owned()),
                report_evidence_ref: None,
            })
            .expect("period");

        let fact = |definition_id: &str, value: &str, confirmation: &str| NewFinancialFact {
            company_id: company.id.clone(),
            period_id: period.id.clone(),
            definition_id: definition_id.to_owned(),
            value_numeric: value.to_owned(),
            currency: Some("PLN".to_owned()),
            statement_basis: None,
            attribution: None,
            variant: None,
            measure_window: None,
            data_quality: None,
            as_reported_value: None,
            as_reported_scale: None,
            reporting_standard: None,
            extraction_method: None,
            confidence: None,
            confirmation_state: Some(confirmation.to_owned()),
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        };
        s.create_financial_fact(fact("kpidef_net_profit", "1250000", "confirmed"))
            .expect("confirmed fact");
        // A not-yet-reviewed fact must NOT appear in the confirmed slice.
        s.create_financial_fact(fact("kpidef_revenue", "9000000", "auto_unreviewed"))
            .expect("unreviewed fact");
    }

    // ---- Properties ---------------------------------------------------------

    /// Small recursive JSON generator so the dispatcher also sees
    /// structurally-valid-but-semantically-wrong messages, not only garbage.
    fn arb_json() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::from),
            any::<i64>().prop_map(Value::from),
            "[a-zA-Z0-9/_.-]{0,12}".prop_map(Value::from),
        ];
        leaf.prop_recursive(3, 24, 6, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..4).prop_map(Value::from),
                proptest::collection::btree_map("[a-z]{1,8}", inner, 0..4)
                    .prop_map(|m| Value::Object(m.into_iter().collect())),
            ]
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        /// The dispatcher must never panic: any bytes/JSON in, `Some(error or
        /// result)` or `None` out.
        #[test]
        fn dispatcher_never_panics_on_arbitrary_json(raw in prop_oneof![
            proptest::string::string_regex(".{0,200}").unwrap(),
            arb_json().prop_map(|v| v.to_string()),
        ]) {
            let response = dispatch(shared_state(), registry::McpScope::Full, &raw);
            if let Some(text) = response {
                let parsed: Value = serde_json::from_str(&text)
                    .expect("every response the dispatcher emits is valid JSON");
                prop_assert!(
                    parsed.get("result").is_some() || parsed.get("error").is_some(),
                    "response must carry result or error: {parsed}"
                );
                prop_assert_eq!(parsed["jsonrpc"].as_str(), Some("2.0"));
            }
        }
    }
}
