import { useEffect, useState } from "react";
import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { listAiProviderCatalog } from "../../api/aiProviders";
import { OPENAI_COMPATIBLE_PROVIDER_ID } from "../../api/credentials";
import type { CapabilityProviderEntry } from "../../api/generated/CapabilityProviderEntry";
import type { AiProviderCatalogEntry } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { ActionRow, Button, FieldRow, Hint, SelectField, TextField } from "../../ui";

type CapabilityRoutingSettingsProps = {
  capabilityProviders: Record<string, CapabilityProviderEntry[]>;
  onCapabilityProvidersChange: (capabilityProviders: Record<string, CapabilityProviderEntry[]>) => void;
};

// Free-text settings fields edit a local draft and commit on blur, rather
// than calling `update_settings` per keystroke (docs/ui-authoring.md): a
// controlled field bound directly to the round-tripped settings value can't
// be typed into, because the async save reverts each keystroke.
type CapabilityMemberModelFieldProps = {
  label: string;
  value: string;
  onCommit: (nextValue: string) => void;
};

function CapabilityMemberModelField({ label, value, onCommit }: CapabilityMemberModelFieldProps) {
  const [draft, setDraft] = useState(value);
  useEffect(() => {
    setDraft(value);
  }, [value]);

  return (
    <TextField
      aria-label={label}
      label={label}
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={() => {
        if (draft !== value) {
          onCommit(draft);
        }
      }}
    />
  );
}

// Fixed set of AI capabilities that can be individually routed to an ordered
// provider fallback pool (ADR 0060 as amended). Keys match `AiCapability::key`
// on the backend — do not rename without updating the Rust enum in lockstep.
export const CAPABILITY_DESCRIPTORS: ReadonlyArray<{ key: string; labelEn: string }> = [
  { key: "kpi_extraction", labelEn: "KPI extraction" },
  { key: "claim_extraction", labelEn: "Claim extraction" },
  { key: "feed_analysis", labelEn: "Feed analysis" },
  { key: "research_brief", labelEn: "Research brief" },
  { key: "research_digest", labelEn: "Research digest" },
  { key: "event_date", labelEn: "Event date" },
  { key: "signal_classification", labelEn: "Signal classification" },
  { key: "qualitative_assessment", labelEn: "Qualitative assessment" }, // T7-C
  { key: "vision_extraction", labelEn: "Vision extraction" }, // ADR 0077 T4.2
];

// Per-capability ordered provider routing (ADR 0060 as amended). Each
// capability gets an ordered failover list of (provider, model) pairs; an
// empty list falls back to the general AI provider configured above.
export function CapabilityRoutingSettings({
  capabilityProviders,
  onCapabilityProvidersChange,
}: CapabilityRoutingSettingsProps) {
  const { text } = useLocale();
  const [catalog, setCatalog] = useState<AiProviderCatalogEntry[]>([]);

  useEffect(() => {
    let active = true;
    listAiProviderCatalog()
      .then((entries) => {
        if (active) {
          setCatalog(entries);
        }
      })
      .catch(() => {
        if (active) {
          setCatalog([]);
        }
      });
    return () => {
      active = false;
    };
  }, []);

  function membersFor(capabilityKey: string): CapabilityProviderEntry[] {
    return capabilityProviders[capabilityKey] ?? [];
  }

  function setMembers(capabilityKey: string, nextMembers: CapabilityProviderEntry[]) {
    onCapabilityProvidersChange({
      ...capabilityProviders,
      [capabilityKey]: nextMembers,
    });
  }

  function addMember(capabilityKey: string) {
    const firstProvider = catalog[0];
    const nextMember: CapabilityProviderEntry = firstProvider
      ? {
          provider: firstProvider.providerId,
          model: firstProvider.providerId === OPENAI_COMPATIBLE_PROVIDER_ID ? "" : firstProvider.defaultModel,
        }
      : { provider: "", model: "" };
    setMembers(capabilityKey, [...membersFor(capabilityKey), nextMember]);
  }

  function updateMember(capabilityKey: string, index: number, nextMember: CapabilityProviderEntry) {
    setMembers(
      capabilityKey,
      membersFor(capabilityKey).map((member, memberIndex) => (memberIndex === index ? nextMember : member)),
    );
  }

  function removeMember(capabilityKey: string, index: number) {
    setMembers(
      capabilityKey,
      membersFor(capabilityKey).filter((_member, memberIndex) => memberIndex !== index),
    );
  }

  function moveMember(capabilityKey: string, index: number, direction: -1 | 1) {
    const members = membersFor(capabilityKey);
    const targetIndex = index + direction;
    if (targetIndex < 0 || targetIndex >= members.length) {
      return;
    }
    const nextMembers = [...members];
    const [moved] = nextMembers.splice(index, 1);
    nextMembers.splice(targetIndex, 0, moved);
    setMembers(capabilityKey, nextMembers);
  }

  return (
    <section
      className="settings-group settings-group--divided"
      aria-labelledby="settings-capability-routing-title"
    >
      <h2 id="settings-capability-routing-title">{text("AI capability routing")}</h2>
      <p className="settings-note">
        {text(
          "Route each AI capability to an ordered provider fallback list. An empty list uses the general AI provider above.",
        )}
      </p>
      <div className="capability-routing-list">
        {CAPABILITY_DESCRIPTORS.map((descriptor) => {
          const capabilityLabel = text(descriptor.labelEn);
          const members = membersFor(descriptor.key);

          return (
            <div className="capability-routing-row" key={descriptor.key}>
              <h3>{capabilityLabel}</h3>
              {members.length === 0 ? (
                <Hint>{text("Uses the general AI provider.")}</Hint>
              ) : null}
              {members.map((member, index) => {
                const providerEntry = catalog.find((entry) => entry.providerId === member.provider);
                const models = providerEntry?.models ?? [];
                const isOpenAiCompatible = member.provider === OPENAI_COMPATIBLE_PROVIDER_ID;
                const memberOrdinal = index + 1;
                const providerLabel = `${text("Provider")} ${capabilityLabel} ${memberOrdinal}`;
                const modelLabel = `${text("Model")} ${capabilityLabel} ${memberOrdinal}`;

                return (
                  <FieldRow className="capability-routing-member" key={index}>
                    <SelectField
                      aria-label={providerLabel}
                      label={providerLabel}
                      value={member.provider}
                      onChange={(event) => {
                        const nextProviderId = event.target.value;
                        const nextProviderEntry = catalog.find((entry) => entry.providerId === nextProviderId);
                        updateMember(descriptor.key, index, {
                          provider: nextProviderId,
                          model: nextProviderEntry?.providerId === OPENAI_COMPATIBLE_PROVIDER_ID
                            ? ""
                            : nextProviderEntry?.defaultModel ?? "",
                        });
                      }}
                    >
                      {catalog.map((entry) => (
                        <option key={entry.providerId} value={entry.providerId}>
                          {entry.label}
                        </option>
                      ))}
                    </SelectField>
                    {isOpenAiCompatible ? (
                      <CapabilityMemberModelField
                        label={modelLabel}
                        value={member.model}
                        onCommit={(nextModel) => {
                          updateMember(descriptor.key, index, { ...member, model: nextModel });
                        }}
                      />
                    ) : (
                      <SelectField
                        aria-label={modelLabel}
                        label={modelLabel}
                        disabled={models.length === 0}
                        value={member.model}
                        onChange={(event) => {
                          updateMember(descriptor.key, index, { ...member, model: event.target.value });
                        }}
                      >
                        {models.length === 0 ? <option value="">{text("Select a provider first")}</option> : null}
                        {models.map((model) => (
                          <option key={model} value={model}>
                            {model}
                          </option>
                        ))}
                      </SelectField>
                    )}
                    <ActionRow>
                      <Button
                        aria-label={`${text("Move up")} ${capabilityLabel} ${memberOrdinal}`}
                        disabled={index === 0}
                        onClick={() => moveMember(descriptor.key, index, -1)}
                        variant="icon"
                      >
                        <ArrowUp size={14} />
                      </Button>
                      <Button
                        aria-label={`${text("Move down")} ${capabilityLabel} ${memberOrdinal}`}
                        disabled={index === members.length - 1}
                        onClick={() => moveMember(descriptor.key, index, 1)}
                        variant="icon"
                      >
                        <ArrowDown size={14} />
                      </Button>
                      <Button
                        aria-label={`${text("Remove")} ${capabilityLabel} ${memberOrdinal}`}
                        onClick={() => removeMember(descriptor.key, index)}
                        variant="icon"
                      >
                        <Trash2 size={14} />
                      </Button>
                    </ActionRow>
                  </FieldRow>
                );
              })}
              <Button onClick={() => addMember(descriptor.key)} variant="secondary">
                <Plus size={14} />
                {text("Add provider")}
              </Button>
            </div>
          );
        })}
      </div>
    </section>
  );
}
