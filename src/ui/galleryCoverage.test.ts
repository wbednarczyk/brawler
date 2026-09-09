import ts from "typescript";
import { describe, expect, it } from "vitest";

// Gate for docs/ui-authoring.md § Adding a new primitive: every VALUE export
// of the src/ui barrel that is a PascalCase component must be rendered as
// real JSX in PrimitiveGallery.tsx or src/ui/gallery/**. TypeScript's own
// parser does the reading, so strings, comments, template and regex literals
// can neither hide a missing demo nor fake one.

const indexModule = import.meta.glob("/src/ui/index.ts", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;
const indexSrc = Object.values(indexModule)[0] ?? "";

const galleryModules = {
  ...(import.meta.glob("/src/ui/PrimitiveGallery.tsx", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>),
  // Empty today unless a demo gets extracted out of PrimitiveGallery.tsx.
  ...(import.meta.glob("/src/ui/gallery/**/*.tsx", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>),
};

function parse(fileName: string, src: string): ts.SourceFile {
  return ts.createSourceFile(fileName, src, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
}

/** The barrel's public component names: `export { A, Inner as B } from …`, values only. */
function extractComponentExports(src: string): string[] {
  const names: string[] = [];
  parse("index.ts", src).forEachChild((node) => {
    if (!ts.isExportDeclaration(node) || node.isTypeOnly) return;
    const clause = node.exportClause;
    if (!clause || !ts.isNamedExports(clause)) return;
    for (const element of clause.elements) {
      if (element.isTypeOnly) continue;
      const name = element.name.text;
      // PascalCase only: excludes `use*` hooks and lowerCamel helpers (both
      // start lowercase) and ALL_CAPS constants (contain "_").
      if (/^[A-Z][a-zA-Z0-9]*$/.test(name)) names.push(name);
    }
  });
  return names;
}

/** Every JSX tag name (`<Name …>` / `<Name />`) that appears in the sources. */
function renderedTagNames(sources: Record<string, string>): Set<string> {
  const tags = new Set<string>();
  const visit = (node: ts.Node) => {
    if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
      tags.add(node.tagName.getText());
    }
    node.forEachChild(visit);
  };
  for (const [fileName, src] of Object.entries(sources)) visit(parse(fileName, src));
  return tags;
}

describe("gallery coverage helpers", () => {
  it("counts only real JSX, never a component name inside a string, template, regex or comment", () => {
    const tags = renderedTagNames({
      "probe.tsx":
        'const a = \'"\'; const b = "<Modal />"; const c = `x ${"`"} <Skeleton />`; // <Subnav />\n' +
        "const r = /<Hint>/; /* <Panel /> */ const d = <Figure value={1} />; const e = <Sparkline points={[]} />;",
    });
    expect([...tags].sort()).toEqual(["Figure", "Sparkline"]);
  });

  it("reads the barrel's public names, including aliased re-exports, and skips hooks/constants/helpers/types", () => {
    const names = extractComponentExports(
      'export { Inner as Public } from "./x";\nexport { useThing, MAX_ROWS, helperFn, Plain } from "./y";\nexport type { PlainProps } from "./y";\nexport { type Shape, Solid } from "./z";',
    );
    expect(names).toEqual(["Public", "Plain", "Solid"]);
  });
});

describe("gallery coverage — every src/ui component export is rendered in PrimitiveGallery", () => {
  const components = extractComponentExports(indexSrc);
  const rendered = renderedTagNames(galleryModules);

  it("found component exports to check (index.ts parsed)", () => {
    expect(components.length).toBeGreaterThan(0);
  });

  it("every component export has a JSX usage in the gallery (docs/ui-authoring.md § Adding a new primitive)", () => {
    const missing = components.filter((name) => !rendered.has(name));
    expect(
      missing,
      `Not rendered in src/ui/PrimitiveGallery.tsx or src/ui/gallery/**: ${missing.join(", ")}. ` +
        "Every src/ui component export must appear in the gallery — see docs/ui-authoring.md § Adding a new primitive.",
    ).toEqual([]);
  });
});
