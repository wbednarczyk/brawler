use super::*;

fn state() -> AppState {
    AppState::new(open_in_memory_database().expect("database should initialize"))
}

#[test]
fn enqueue_is_idempotent_by_id() {
    let jobs = state().jobs();

    assert!(jobs
        .enqueue("dup", "kind-a", "{}", 3)
        .expect("first enqueue"));
    // Same id -> dedup, no second row.
    assert!(!jobs
        .enqueue("dup", "kind-a", "{}", 3)
        .expect("second enqueue"));

    let counts = jobs.counts().expect("counts");
    assert_eq!(counts.pending, 1);
}

#[test]
fn claim_returns_none_when_empty() {
    let jobs = state().jobs();
    assert!(jobs.claim_next().expect("claim").is_none());
}

#[test]
fn claim_moves_to_running_and_increments_attempts() {
    let jobs = state().jobs();
    jobs.enqueue("job", "kind-a", "{\"x\":1}", 3)
        .expect("enqueue");

    let claimed = jobs.claim_next().expect("claim").expect("a job");
    assert_eq!(claimed.id, "job");
    assert_eq!(claimed.kind, "kind-a");
    assert_eq!(claimed.payload, "{\"x\":1}");
    assert_eq!(claimed.attempts, 1);
    assert_eq!(claimed.max_attempts, 3);

    // A claimed job is no longer claimable.
    assert!(jobs.claim_next().expect("claim").is_none());
    assert_eq!(jobs.counts().expect("counts").running, 1);
}

#[test]
fn mark_succeeded_is_terminal() {
    let jobs = state().jobs();
    jobs.enqueue("job", "kind-a", "{}", 3).expect("enqueue");
    jobs.claim_next().expect("claim").expect("a job");

    jobs.mark_succeeded("job").expect("succeed");
    let counts = jobs.counts().expect("counts");
    assert_eq!(counts.succeeded, 1);
    assert_eq!(counts.running, 0);
    assert_eq!(counts.pending, 0);
}

#[test]
fn mark_failed_retries_until_max_then_fails_terminally() {
    let jobs = state().jobs();
    jobs.enqueue("job", "kind-a", "{}", 2).expect("enqueue");

    // Attempt 1 -> retryable (attempts 1 < max 2): back to pending.
    jobs.claim_next().expect("claim").expect("a job");
    let retried = jobs.mark_failed("job", "boom", 0).expect("fail 1");
    assert!(retried);
    assert_eq!(jobs.counts().expect("counts").pending, 1);

    // Attempt 2 -> attempts now 2 == max: terminal failure.
    jobs.claim_next().expect("claim").expect("a job");
    let retried = jobs.mark_failed("job", "boom again", 0).expect("fail 2");
    assert!(!retried);
    let counts = jobs.counts().expect("counts");
    assert_eq!(counts.failed, 1);
    assert_eq!(counts.pending, 0);
}

#[test]
fn reclaim_requeues_running_rows() {
    let jobs = state().jobs();
    jobs.enqueue("a", "kind-a", "{}", 3).expect("enqueue a");
    jobs.enqueue("b", "kind-a", "{}", 3).expect("enqueue b");

    jobs.claim_next().expect("claim").expect("a job");
    jobs.claim_next().expect("claim").expect("b job");
    assert_eq!(jobs.counts().expect("counts").running, 2);

    let reclaimed = jobs.reclaim_stale_running().expect("reclaim");
    assert_eq!(reclaimed, 2);
    let counts = jobs.counts().expect("counts");
    assert_eq!(counts.running, 0);
    assert_eq!(counts.pending, 2);
}
