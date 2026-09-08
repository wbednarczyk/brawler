//! Per-document capture lock (#487 P1 r3, adversarial review on the
//! atomicity-wave PR): [`super::capture_report_document`],
//! [`super::fetch_report_document`], and [`super::fetch_pending_attachments`]
//! hold this lock across a document's entire capture body (inspect →
//! resolve/heal/reset → fetch → store → row write), so a second caller for
//! the same document waits and then observes the first caller's published
//! row through the normal `fetch_status == "fetched"` short-circuit instead
//! of interleaving with it. Compare-and-set on the row alone cannot close
//! this: it can arbitrate who *wins* a race, but it cannot see a concurrent
//! REPAIR that restores the identical bytes to the same path between another
//! caller's inspection and its own write — only serializing the whole
//! capture closes that window.
//!
//! Brawler is a single-instance process (the Tauri single-instance plugin
//! plus the in-process MCP server both run inside the one app process), so
//! this in-process lock is the complete solution — no cross-process lock
//! file or advisory lock is needed.
//!
//! Usage: `let doc_lock = doc_lock::lock_document(&doc_id); let _guard =
//! doc_lock.lock().unwrap_or_else(|p| p.into_inner());` — keep both bindings
//! alive for the whole capture body; the guard borrows through the `Arc`, so
//! it must not outlive it.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

static DOC_LOCKS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Returns the mutex serializing captures of report document `id`, creating
/// it on first use.
///
/// ponytail: entries are never evicted — one `Arc<Mutex<()>>` per document id
/// ever captured, for the life of the process. Bounded by document count
/// (thousands, not millions) and negligible per entry; add `Weak`-based
/// cleanup if that stops holding.
pub(crate) fn lock_document(id: &str) -> Arc<Mutex<()>> {
    let mut locks = DOC_LOCKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Arc::clone(
        locks
            .entry(id.to_owned())
            .or_insert_with(|| Arc::new(Mutex::new(()))),
    )
}
