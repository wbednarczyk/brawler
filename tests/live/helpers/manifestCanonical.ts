// Canonicalize a `validate_kpi_ingest` manifest so two SEQUENTIAL runs of the
// SAME document with the SAME observations can be deep-compared (#389 Noga C).
//
// Two such runs differ ONLY by run identity: the manifest carries `runId`, and
// every restage mints fresh `observationId`s (`kpiobs_*`) that appear both on
// `observations[]` and inside `runDiagnostics[].detail.observationIds`. This
// canonicalizer maps every `observationId` to a stable `ordinal:<n>`, drops the
// top-level `runId`, and sorts observations by ordinal. EVERYTHING else —
// `reportDocumentId`, `sourceContentHash`, `companyId`, `period`, `expectedKpis`,
// `validatorVersion`, per-observation content / `validationState` / `codes`,
// `runDiagnostics`, `completeness`, `counts`, `outcome` — is preserved and MUST
// match (same document ⇒ same doc id and pinned-blob hash). A deep-equal of two
// canonicalized manifests therefore proves the SERVER reached the same verdict
// regardless of which run/driver produced it (server invariance, not client
// interop — see the spec's honest scoping).
//
// `revision` is preserved: both compared runs are validated at the same revision
// (each run's first `validate`), so it must be equal; a real difference should
// fail rather than be hidden.

type Json = null | boolean | number | string | Json[] | { [k: string]: Json };

export function canonicalizeManifest(manifest: Json): Json {
  const root = manifest as { observations?: Array<{ observationId?: string; ordinal?: number }> };
  const idToOrdinal = new Map<string, number>();
  for (const observation of root.observations ?? []) {
    if (typeof observation.observationId === "string" && typeof observation.ordinal === "number") {
      idToOrdinal.set(observation.observationId, observation.ordinal);
    }
  }

  const walk = (node: Json): Json => {
    if (Array.isArray(node)) return node.map(walk);
    if (node !== null && typeof node === "object") {
      const out: { [k: string]: Json } = {};
      for (const [key, value] of Object.entries(node)) {
        if (key === "runId") continue; // volatile run identity
        out[key] = walk(value);
      }
      return out;
    }
    if (typeof node === "string") {
      const ordinal = idToOrdinal.get(node);
      return ordinal === undefined ? node : `ordinal:${ordinal}`;
    }
    return node;
  };

  const canon = walk(manifest) as { observations?: Array<{ ordinal?: number }> };
  if (Array.isArray(canon.observations)) {
    canon.observations.sort((a, b) => (a.ordinal ?? 0) - (b.ordinal ?? 0));
  }
  return canon as Json;
}
