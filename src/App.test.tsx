import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({ status: "ok", version: "0.1.0" }),
}));

describe("App", () => {
  it("renders the investor inbox shell", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByText("GPW:CDR")).toBeInTheDocument();
    expect(screen.getByText("Selected item")).toBeInTheDocument();
    expect(await screen.findByText("ok 0.1.0")).toBeInTheDocument();
  });
});
