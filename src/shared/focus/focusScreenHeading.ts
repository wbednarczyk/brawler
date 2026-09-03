// Keyboard navigation must never leave focus on `<body>` (ADR 0076 dec. 9
// amendment, F3c): after a palette command or a guarded Spółka transition
// whose intent moves nothing, a screen change would otherwise strand focus
// there (the Modal restores to a removed invoker → skipped → body). The
// fallback destination is the active screen's first heading — a non-input
// element, so Chromium paints `:focus-visible` only after a keyboard
// interaction, never for mouse users.
export function focusScreenHeadingIfBody(): boolean {
  if (typeof document === "undefined") return false;
  if (document.activeElement && document.activeElement !== document.body) return false;
  const heading = document.querySelector<HTMLElement>("main.workspace h1, main.workspace h2");
  if (!heading) return false;
  if (!heading.hasAttribute("tabindex")) heading.setAttribute("tabindex", "-1");
  heading.focus();
  return document.activeElement === heading;
}
