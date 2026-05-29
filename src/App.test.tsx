import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((command: string) => {
    if (command === "health") {
      return Promise.resolve({ status: "ok", version: "0.1.0" });
    }

    if (command === "database_status") {
      return Promise.resolve({
        appliedMigrations: 1,
        companies: 0,
        sourceAdapters: 1,
        settings: 7,
      });
    }

    return Promise.reject(new Error(`Unexpected command: ${command}`));
  }),
}));

describe("App", () => {
  it("renders the investor inbox shell", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByText("GPW:CDR")).toBeInTheDocument();
    expect(screen.getByText("Selected item")).toBeInTheDocument();
    expect(await screen.findByText("ok 0.1.0")).toBeInTheDocument();
    expect(await screen.findByText("1 migration, 1 source, 7 settings")).toBeInTheDocument();
    expect(screen.getByLabelText("Database connection active")).toBeInTheDocument();
  });
});
