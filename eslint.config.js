import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
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

const PRIMITIVE_FIRST = [
  "error",
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
];

export default tseslint.config(
  {
    ignores: [
      "dist/**",
      "src-tauri/**",
      "playwright-report/**",
      "test-results/**",
      "coverage/**",
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
    // The primitive-first ban applies to authored screens/components only —
    // not the primitives themselves (which legitimately wrap native elements)
    // and not tests (which assert against native DOM).
    files: ["src/**/*.tsx"],
    ignores: ["src/ui/**", "**/*.test.tsx", "src/test/**"],
    rules: {
      "no-restricted-syntax": PRIMITIVE_FIRST,
    },
  },
);
