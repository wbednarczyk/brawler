-- Qualitative, agent-assessed criteria (v0.50.0, ADR 0075). Append-only extension
-- of the quality-frameworks schema (migration 0048): framework_criteria gains a
-- kind discriminator + an owner-authored assessment_guidance seed; criterion_results
-- gains the agent-assessed result fields. Every column carries a safe default so the
-- existing quantitative rows stay valid on upgrade (kind='quantitative',
-- source='engine'); the agent-only fields are nullable. Append-only + idempotent:
-- the runner applies each version exactly once (schema_migrations bookkeeping).

-- A criterion is quantitative (DSL over metric keys) or qualitative (agent-assessed,
-- guided by an owner-authored prompt seed; no DSL expression). NOT NULL DEFAULT keeps
-- pre-migration rows valid without a data pass.
ALTER TABLE framework_criteria ADD COLUMN kind TEXT NOT NULL DEFAULT 'quantitative';
ALTER TABLE framework_criteria ADD COLUMN assessment_guidance TEXT;

-- Agent-assessed result fields (populated only for source='agent' rows, ADR 0075).
-- reasoning: short rationale; citations: JSON array of typed evidence refs
-- (evidenceType/evidenceId/label/snippet); confidence: low|medium|high;
-- prompt_version: the versioned prompt id; source: engine (quantitative) | agent.
ALTER TABLE criterion_results ADD COLUMN reasoning TEXT;
ALTER TABLE criterion_results ADD COLUMN citations TEXT;
ALTER TABLE criterion_results ADD COLUMN confidence TEXT;
ALTER TABLE criterion_results ADD COLUMN prompt_version TEXT;
ALTER TABLE criterion_results ADD COLUMN source TEXT NOT NULL DEFAULT 'engine';
