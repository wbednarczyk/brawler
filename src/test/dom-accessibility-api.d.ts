// The package ships an ESM build without a resolvable declaration under this
// project's module resolution; only the one function the contract helper uses.
declare module "dom-accessibility-api" {
  export function computeAccessibleName(element: Element): string;
}
