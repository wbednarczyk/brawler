import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import testingLibrary from "eslint-plugin-testing-library";
import globals from "globals";

// Primitive-first authoring contract (ADR 0037, docs/ui-authoring.md).
// Raw <input>/<select>/<textarea> and inline style={{…}} are banned in
// screens/components — compose from the src/ui primitives instead. The ban is
// deliberately escapable: a genuinely-native control that no primitive covers
// (a checkbox, a file/date/time picker, a ref-bound or keyboard-driven widget)
// is allowed, either because its input `type` is inherently native (exempted
// below) or via an inline `// eslint-disable-next-line no-restricted-syntax --
// <reason>` that documents why at the call site. This keeps the rule strict
// without preventing anything that should be allowed.

// Input types that have no primitive equivalent and are always allowed raw.
const NATIVE_INPUT_TYPES = "checkbox|radio|file|date|time|datetime-local|month|week|range|color";

// Feedback policy (ADR 0076 Decision 5): no native window.confirm() outside
// src/ui. A reversible destroy runs immediately with a `Cofnij` toast
// (useUndoableDelete); an irreversible/cascading one uses the InlineConfirm
// primitive. Native dialogs are unstyled, untranslatable, and untestable
// without dialog auto-accept plumbing. Matches `window.confirm` specifically so
// a legitimately-named local `confirm(...)` helper is not flagged.
const CONFIRM_BAN = {
  selector: "MemberExpression[object.name='window'][property.name='confirm']",
  message:
    "No native window.confirm() (ADR 0076 Decision 5). Use useUndoableDelete + Toast for reversible deletes, or the InlineConfirm primitive for irreversible/cascading actions.",
};

const PRIMITIVE_FIRST = [
  "error",
  CONFIRM_BAN,
  {
    selector: "JSXOpeningElement[name.name='select']",
    message:
      "Use the SelectField primitive instead of a raw <select> (docs/ui-authoring.md). If a native <select> is genuinely required, add `// eslint-disable-next-line no-restricted-syntax -- <reason>`.",
  },
  {
    selector: "JSXOpeningElement[name.name='textarea']",
    message:
      "Use the TextareaField primitive instead of a raw <textarea> (docs/ui-authoring.md). If a native control is genuinely required, add `// eslint-disable-next-line no-restricted-syntax -- <reason>`.",
  },
  {
    // Flag raw <input> unless its type is an inherently-native one (checkbox,
    // file, date/time picker, …). Those have no primitive and are always allowed.
    selector: `JSXOpeningElement[name.name='input']:not(:has(JSXAttribute[name.name='type'] > Literal[value=/^(${NATIVE_INPUT_TYPES})$/]))`,
    message:
      "Use the TextField/SearchField primitive instead of a raw text <input> (docs/ui-authoring.md). Native checkbox/radio/file/date/time inputs are already allowed; for another genuinely-native control add `// eslint-disable-next-line no-restricted-syntax -- <reason>`.",
  },
  {
    // Catch both `style={{…}}` and `style={{…} as CSSProperties}` (the `as`
    // cast wraps the object literal in a TSAsExpression).
    selector:
      "JSXAttribute[name.name='style'] > JSXExpressionContainer > ObjectExpression, JSXAttribute[name.name='style'] > JSXExpressionContainer > TSAsExpression > ObjectExpression",
    message:
      "No inline style={{…}} outside src/ui (ADR 0037). Put containment/spacing in CSS or bake it into a primitive. If unavoidable, add `// eslint-disable-next-line no-restricted-syntax -- <reason>`.",
  },
  {
    // A raw HTML element (lowercase tag) with the `error-text` class is the
    // ad-hoc error line the ErrorText primitive replaces. Component elements
    // (<ErrorText>, which renders the class itself) start uppercase and are not
    // matched. For an inline error inside a row use <ErrorText as="span">.
    selector:
      "JSXOpeningElement[name.name=/^[a-z]/] > JSXAttribute[name.name='className'][value.value=/(^|\\s)error-text(\\s|$)/]",
    message:
      "Use the ErrorText primitive instead of a raw element with className=\"error-text\" (docs/ui-authoring.md). For an inline error use <ErrorText as=\"span\">.",
  },
];

// Barrel discipline: consumers import primitives from the "…/ui" barrel
// (src/ui/index.ts, the public surface), never a deep "…/ui/Button" path.
const BARREL_PATTERN = {
  group: ["**/ui/*", "!**/ui/index"],
  message:
    'Import UI primitives from the barrel (e.g. `from "../../ui"`), not a deep path (`../../ui/Button`). The barrel src/ui/index.ts is the public surface.',
};

// dockview containment (ADR 0053): dockview backs the research-cockpit shell
// spike. Its imports are banned everywhere EXCEPT src/screens/Cockpit/** (the
// exemption block below). It must not leak into another screen until ADR 0053 is
// Accepted and this boundary is widened deliberately.
const DOCKVIEW_RESTRICTION = {
  group: ["dockview", "dockview-core", "dockview-react", "dockview/*"],
  message:
    "dockview is confined to the research-cockpit spike (src/screens/Cockpit/) pending ADR 0053 acceptance. Do not import it elsewhere without flipping ADR 0053 to Accepted and widening this rule.",
};

// Frontend layer contract (issue #50; canonical statement in
// docs/modularization-design.md § Frontend layer contract). Enforced edges:
// - src/api is the IPC bottom: it imports NO app-side layer.
// - src/shared is reusable across screens: it never imports app/ or screens/
//   (the composition roots).
// - src/ui primitives are presentation-only: no app state, screens, or IPC;
//   the only sanctioned shared leaves are locale + format (display helpers)
//   + verbs (the ADR 0104 dec. 3 verb dictionary, F4a S1 — `ActionButton`'s
//   `Verb` type; a single dependency-free display-string table, same category
//   as locale/format).
// src/ui is a flat directory, so its cross-layer imports are exactly one
// `../<layer>/…` hop — anchored patterns, immune to npm-specifier collisions
// (e.g. "@tauri-apps/api/…" must NOT match an "api" layer ban).
const UI_LAYER_RESTRICTIONS = [
  {
    group: ["../app/**", "../screens/**", "../api/**", "../App", "../main"],
    message:
      "src/ui primitives are presentation-only — no app state, screens, or IPC imports (docs/modularization-design.md § Frontend layer contract).",
  },
  {
    // Everything under ../shared/ EXCEPT the sanctioned display leaves
    // shared/locale, shared/format, and shared/verbs (negated `group` globs
    // do not compose with the `../` prefix, hence the regex form).
    regex: "^\\.\\./shared/(?!locale($|/)|format/|verbs$)",
    message:
      "src/ui may reach only the sanctioned shared leaves: shared/locale, shared/format (display helpers), and shared/verbs (the verb dictionary). Anything else belongs above the primitive layer (docs/modularization-design.md § Frontend layer contract).",
  },
];

const SHARED_LAYER_RESTRICTION = {
  group: ["**/app/**", "**/screens/**"],
  message:
    "src/shared is reusable across screens — it must not import the composition roots (src/app, src/screens). Pass data/handlers in via props or a context the composer provides (docs/modularization-design.md § Frontend layer contract).",
};

const API_LAYER_RESTRICTION = {
  group: ["../app/**", "../screens/**", "../shared/**", "../ui/**", "../App", "../main"],
  message:
    "src/api is the IPC bottom layer — it imports only its own modules and the Tauri API, never app/screens/shared/ui (docs/modularization-design.md § Frontend layer contract).",
};

// Test hygiene (G10, docs/testing.md § Frontend test responsibilities): a
// committed `.only()` silently skips the rest of the suite — it must never
// reach the gate. One selector catches it.only/test.only/describe.only AND
// the nested test.describe.only (its outer call's callee.property is also
// "only", so the same selector matches all four spellings from CLAUDE.md).
const ONLY_BAN = {
  selector: "CallExpression[callee.type='MemberExpression'][callee.property.name='only']",
  message:
    'No .only() in committed tests — it.only/test.only/describe.only/test.describe.only silently skips the rest of the suite (docs/testing.md § Frontend test responsibilities).',
};

// G11 (dogfooding #11): a `data-*-highlighted`/`-marked` attribute is not
// proof of a visible mark — CompanyReportDocumentsPanel set
// `data-document-highlighted` for 4s with no CSS rule ever painting it, and
// the browser spec that asserted the attribute stayed green throughout.
// Scoped to tests/** (Playwright, a real layout engine) only: jsdom computes
// no colors (docs/testing.md § Frontend test responsibilities), so the
// equivalent Vitest assertions in src/**/*.test.* (App.test.tsx,
// CompanyReportDocumentsPanel.test.tsx) are legitimate and must not be
// flagged (ADR 0045 — never flag legitimate code). Narrowed to
// highlighted/marked only (dropped selected/active, sol diff finding 8):
// `data-selected`/`data-active` commonly track pure logic/ARIA state with no
// visual-mark claim, so banning them flagged legitimate assertions.
// `-highlighted`/`-marked` name the paint claim itself (matches
// `data-document-highlighted`, the real #11 case). Matches both a plain
// string literal and a no-substitution template literal first argument.
const PAINT_ATTR_REGEX = "/^data-([a-z-]*-)?(highlighted|marked)$/";
// (b) fix (Astra re-verification of PR #477 finding 8): the TemplateLiteral
// branch only ever looked at the first quasi, so
// `` `data-highlighted${suffix}` `` was banned even though a non-empty
// `suffix` (e.g. "-reason") can make the resulting attribute name legitimate.
// Restrict the TemplateLiteral branch to a literal with ZERO expressions
// (`expressions.length=0`) — a plain no-substitution template — so an
// interpolated attribute name is never matched by this selector at all.
const TO_HAVE_ATTRIBUTE_PAINT_BAN = {
  selector:
    `CallExpression[callee.property.name='toHaveAttribute'][arguments.0.type='Literal'][arguments.0.value=${PAINT_ATTR_REGEX}], ` +
    `CallExpression[callee.property.name='toHaveAttribute'][arguments.0.type='TemplateLiteral'][arguments.0.expressions.length=0][arguments.0.quasis.0.value.raw=${PAINT_ATTR_REGEX}]`,
  message: "assert the paint via tests/browser/helpers/paint.ts, not the attribute (dogfooding #11)",
};

// T3 hard gates wave 2 harvest (2026-09-08): `openPalette(page)` in
// tests/browser/helpers/harness.ts is the one place every spec is meant to
// open the ⌘K palette (blur-then-retry against a slow CI runner, PR #432/
// #452) — the doc comment said so, but 25 call sites pressed `Control+K`
// bare anyway, and one flaked in CI once retries dropped to 0
// (j8-keyboard-only-company-review.spec.ts). Ban the literal everywhere
// under tests/** except harness.ts itself (the block below carves out that
// exemption via `ignores`).
const CONTROL_K_BAN = {
  selector: "Literal[value='Control+K']",
  message:
    "Open the ⌘K palette via openPalette(page) from helpers/harness — a bare Control+K races the shortcut listener on a slow runner (retries are 0).",
};

// it.only.each/test.only.each (sol diff finding 8): `.only` one level below
// the top call is invisible to ONLY_BAN below (its callee.property is
// `each`, not `only`) but still silently narrows the suite to one
// parameterized case.
const ONLY_EACH_BAN = {
  selector:
    "CallExpression[callee.type='MemberExpression'][callee.object.type='MemberExpression'][callee.object.property.name='only']",
  message:
    "No .only.each() in committed tests — it.only.each/test.only.each silently narrows the suite to one parameterized case (docs/testing.md § Frontend test responsibilities).",
};

// Skip/fixme/todo hygiene (G10): an undocumented skip is a silently-shrinking
// suite. Two small local rules (no plugin dependency) rather than a single
// no-restricted-syntax selector, because "does it carry a reason" needs
// argument-shape/comment inspection that esquery selectors can't express.
const testHygienePlugin = {
  rules: {
    // Vitest: it.skip/test.skip/describe.skip/it.todo/test.todo take no
    // reason slot, so the reason lives in a preceding comment instead.
    "skip-needs-reason-comment": {
      meta: { type: "problem" },
      create(context) {
        const sourceCode = context.sourceCode ?? context.getSourceCode();
        return {
          CallExpression(node) {
            const callee = node.callee;
            if (callee.type !== "MemberExpression" || callee.property.type !== "Identifier") return;
            if (!["skip", "todo", "fixme"].includes(callee.property.name)) return;
            const hasReason = sourceCode
              .getCommentsBefore(node)
              .some((comment) => /skip-ok:\s*\S/.test(comment.value));
            if (!hasReason) {
              context.report({
                node,
                message:
                  "Vitest .skip()/.todo() needs a preceding `// skip-ok: <reason>` comment (docs/testing.md § Frontend test responsibilities).",
              });
            }
          },
        };
      },
    },
    // Playwright: test.skip/.fixme/.todo take the reason as their last
    // argument (test.skip(condition, "reason")). Restricted to a direct
    // `test.<method>(...)`/`it.<method>(...)` callee (sol diff finding 8):
    // `test.describe.skip(...)`/`.fixme(...)` have no reason slot at all (a
    // different, comment-based hygiene question this rule must not flag),
    // and an unrelated `.skip(n)`/`.fixme(...)` call on some other object
    // (e.g. an iterator/stream helper) is not a test skip at all.
    "skip-needs-reason-arg": {
      meta: { type: "problem" },
      create(context) {
        return {
          CallExpression(node) {
            const callee = node.callee;
            if (callee.type !== "MemberExpression" || callee.property.type !== "Identifier") return;
            if (!["skip", "fixme", "todo"].includes(callee.property.name)) return;
            if (callee.object.type !== "Identifier" || !["test", "it"].includes(callee.object.name)) return;
            const args = node.arguments;
            const last = args[args.length - 1];
            const hasStringReason = last && last.type === "Literal" && typeof last.value === "string";
            if (!hasStringReason) {
              context.report({
                node,
                message:
                  "Playwright test.skip()/.fixme()/.todo() needs a string reason as the last argument (docs/testing.md § Frontend test responsibilities).",
              });
            }
          },
        };
      },
    },
  },
};

export default tseslint.config(
  {
    ignores: [
      "dist/**",
      "src-tauri/**",
      "playwright-report/**",
      "test-results/**",
      "coverage/**",
      "src/api/generated/**", // ts-rs-generated DTOs (ADR 0048); regenerate, don't hand-edit
      "node_modules/**",
      "**/*.config.js",
      "**/*.config.ts",
    ],
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    languageOptions: {
      globals: { ...globals.browser, ...globals.es2021 },
    },
    plugins: { "react-hooks": reactHooks },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",
      // Surface, don't block: these are useful signals but not part of this
      // epic's scope to drive to zero, so they warn rather than fail the gate.
      "@typescript-eslint/no-explicit-any": "warn",
      "@typescript-eslint/no-unused-vars": ["warn", { argsIgnorePattern: "^_", varsIgnorePattern: "^_" }],
    },
  },
  {
    // Guardrail for the async-flush test class the React 19 migration exposed
    // (issue #262): a sync query — or an unawaited event/find — racing a render
    // React 18 used to flush synchronously. These three rules make the racy
    // idioms unwritable; `prefer-find-by` additionally collapses
    // waitFor+getBy* into the equivalent findBy*.
    files: ["src/**/*.test.{ts,tsx}", "src/test/**/*.{ts,tsx}"],
    plugins: { "testing-library": testingLibrary, local: testHygienePlugin },
    rules: {
      "testing-library/await-async-queries": "error",
      // userEvent only: React's fireEvent is synchronous by design.
      "testing-library/await-async-events": ["error", { eventModule: "userEvent" }],
      "testing-library/prefer-find-by": "error",
      "no-restricted-syntax": ["error", ONLY_BAN, ONLY_EACH_BAN],
      "local/skip-needs-reason-comment": "error",
    },
  },
  {
    // Playwright browser/live specs (G10/G11, docs/testing.md § Frontend test
    // responsibilities). Not a subset of the "src/**" block above, so it gets
    // its own minimal parser wiring rather than the full js/tseslint
    // recommended sets — this stays a targeted hygiene gate, not a new lint
    // surface over 50+ existing spec files (ADR 0045: precise gates only).
    // Excludes tests/browser/**: CONTROL_K_BAN (below) is scoped there only
    // — tests/live/** drives the real app over `./helpers/liveConnect`, a
    // different runtime with no `openPalette` to route through, so banning
    // the literal there would flag legitimate code (ADR 0045).
    files: ["tests/**/*.ts"],
    ignores: ["tests/browser/**/*.ts"],
    languageOptions: { parser: tseslint.parser },
    plugins: { local: testHygienePlugin },
    rules: {
      "no-restricted-syntax": ["error", ONLY_BAN, ONLY_EACH_BAN, TO_HAVE_ATTRIBUTE_PAINT_BAN],
      "local/skip-needs-reason-arg": "error",
    },
  },
  {
    // tests/browser/**: same hygiene bans, plus CONTROL_K_BAN — every spec
    // here can reach `openPalette` (helpers/harness.ts), the one place meant
    // to open the ⌘K palette (blur-then-retry against a slow CI runner, PR
    // #432/#452) — the doc comment said so, but 25 call sites pressed
    // `Control+K` bare anyway, and one flaked in CI once retries dropped to 0
    // (j8-keyboard-only-company-review.spec.ts, T3 hard gates wave 2,
    // 2026-09-08). Excludes harness.ts itself, the one legitimate site for
    // the literal (split into a third block below rather than an `ignores`
    // on this one, so harness.ts keeps the other three bans — flat config
    // REPLACES, not merges, a rule across matching blocks, so the exempt
    // file needs the full non-Control+K array repeated, not just a
    // subtraction).
    files: ["tests/browser/**/*.ts"],
    ignores: ["tests/browser/helpers/harness.ts"],
    languageOptions: { parser: tseslint.parser },
    plugins: { local: testHygienePlugin },
    rules: {
      "no-restricted-syntax": ["error", ONLY_BAN, ONLY_EACH_BAN, TO_HAVE_ATTRIBUTE_PAINT_BAN, CONTROL_K_BAN],
      "local/skip-needs-reason-arg": "error",
    },
  },
  {
    // harness.ts itself: same hygiene bans, minus CONTROL_K_BAN (see above).
    files: ["tests/browser/helpers/harness.ts"],
    languageOptions: { parser: tseslint.parser },
    plugins: { local: testHygienePlugin },
    rules: {
      "no-restricted-syntax": ["error", ONLY_BAN, ONLY_EACH_BAN, TO_HAVE_ATTRIBUTE_PAINT_BAN],
      "local/skip-needs-reason-arg": "error",
    },
  },
  {
    // The primitive-first ban applies to authored screens/components only —
    // not the primitives themselves (which legitimately wrap native elements)
    // and not tests (which assert against native DOM).
    files: ["src/**/*.tsx"],
    ignores: ["src/ui/**", "**/*.test.tsx", "src/test/**"],
    rules: {
      "no-restricted-syntax": PRIMITIVE_FIRST,
    },
  },
  {
    // The window.confirm ban also reaches plain .ts controllers (a separate
    // block so it does not clobber PRIMITIVE_FIRST on .tsx — flat config
    // replaces rather than merges a rule across matching blocks).
    files: ["src/**/*.ts"],
    ignores: ["src/ui/**", "**/*.test.ts", "src/test/**"],
    rules: {
      "no-restricted-syntax": ["error", CONFIRM_BAN],
    },
  },
  {
    // Barrel discipline + dockview containment. src/ui/** (siblings import each
    // other relatively) and src/gallery.tsx (the dev-only gallery deep-imports
    // the intentionally-unexported PrimitiveGallery) are exempt from the barrel
    // rule. dockview is banned here and allowed only in the Companies block below.
    files: ["src/**/*.{ts,tsx}"],
    ignores: ["src/ui/**", "src/gallery.tsx"],
    rules: {
      "no-restricted-imports": ["error", { patterns: [BARREL_PATTERN, DOCKVIEW_RESTRICTION] }],
    },
  },
  {
    // dockview spike exemption (ADR 0053): the research cockpit is the only
    // place allowed to import dockview. Barrel discipline still applies, so this
    // block restates BARREL_PATTERN (flat config replaces — not merges — a rule
    // across matching blocks) while dropping the dockview ban.
    files: ["src/screens/Cockpit/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-imports": ["error", { patterns: [BARREL_PATTERN] }],
    },
  },
  // Layer-contract blocks (issue #50). Flat config REPLACES a rule across
  // matching blocks, so each block restates the generic patterns that still
  // apply to its files (barrel + dockview) alongside its layer bans.
  {
    files: ["src/ui/**/*.{ts,tsx}"],
    ignores: ["**/*.test.{ts,tsx}"],
    rules: {
      // src/ui is exempt from BARREL_PATTERN (siblings import relatively);
      // dockview stays banned here.
      "no-restricted-imports": [
        "error",
        { patterns: [DOCKVIEW_RESTRICTION, ...UI_LAYER_RESTRICTIONS] },
      ],
    },
  },
  {
    files: ["src/shared/**/*.{ts,tsx}"],
    ignores: ["**/*.test.{ts,tsx}"],
    rules: {
      "no-restricted-imports": [
        "error",
        { patterns: [BARREL_PATTERN, DOCKVIEW_RESTRICTION, SHARED_LAYER_RESTRICTION] },
      ],
    },
  },
  {
    files: ["src/api/**/*.ts"],
    ignores: ["**/*.test.ts"],
    rules: {
      "no-restricted-imports": [
        "error",
        { patterns: [BARREL_PATTERN, DOCKVIEW_RESTRICTION, API_LAYER_RESTRICTION] },
      ],
    },
  },
);
