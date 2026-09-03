// ADR 0076 Decisions 1–2 — spacing/type token gate.
//
// Mechanism: `declaration-property-value-allowed-list` (the rule the ADR names).
// It expresses the intent cleanly because we anchor a per-property regex to the
// WHOLE declaration value: raw px/rem, ad-hoc numbers, and unknown units are all
// rejected, while the sanctioned set passes — spacing/type tokens, `0`, `auto`,
// `%`, `calc(-1 * var(--space-N))` for hairline negative margins, and the CSS
// keywords `inherit`/`unset`/`normal`. Verified to yield zero false positives
// against the fully migrated src/**/*.css (U2). tokens.css defines the tokens
// themselves (px literals) but declares none of the gated properties, so it needs
// no exception; themes.css declares none either.
const spaceComp = String.raw`(?:var\(--space-[0-8]\)|calc\(-1 \* var\(--space-[0-8]\)\)|0|auto|[0-9.]+%)`;
const spaceValue = new RegExp(`^(?:inherit|unset|normal|${spaceComp}(?: ${spaceComp}){0,3})$`);
const fontValue = /^(?:var\(--font-(?:2xs|xs|sm|base|md|lg|xl)\)|inherit|unset)$/;

const spacingProps = [
  "gap",
  "row-gap",
  "column-gap",
  "padding",
  "padding-top",
  "padding-right",
  "padding-bottom",
  "padding-left",
  "margin",
  "margin-top",
  "margin-right",
  "margin-bottom",
  "margin-left",
];

const allowedList = { "font-size": [fontValue] };
for (const prop of spacingProps) {
  allowedList[prop] = [spaceValue];
}

export default {
  rules: {
    "color-no-hex": true,
    "no-duplicate-selectors": true,
    "block-no-empty": true,
    "declaration-block-no-duplicate-properties": true,
    "no-invalid-double-slash-comments": true,
    "declaration-property-value-allowed-list": allowedList,
    // Global focus ring (F3c S2, #197, plan § Design 7): a suppressed outline
    // hides keyboard focus. `:focus-visible { outline: 2px solid var(--primary) }`
    // in ui.css is the one ring; a wrapper that needs a `:focus-within` ring
    // instead still must not disable the input's own outline (ui-authoring.md).
    "declaration-property-value-disallowed-list": { outline: ["none", "0"] },
  },
  overrides: [
    {
      files: ["src/styles/tokens.css", "src/styles/themes.css"],
      rules: { "color-no-hex": null },
    },
  ],
};
