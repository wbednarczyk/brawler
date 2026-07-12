import { describe, expect, it, vi, beforeEach } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: unknown) => invokeMock(command, args),
}));

import {
  callCommand,
  isCommandError,
  CommandInvocationError,
} from "./tauri";

describe("callCommand CommandError envelope (ADR 0070)", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("resolves the command result unchanged on success", async () => {
    invokeMock.mockResolvedValueOnce({ ok: true });
    await expect(callCommand("some_command")).resolves.toEqual({ ok: true });
  });

  it("rejects with the original string for legacy string errors", async () => {
    invokeMock.mockRejectedValueOnce("plain legacy failure");
    await expect(callCommand("legacy_command")).rejects.toBe(
      "plain legacy failure",
    );
  });

  it("surfaces a typed CommandInvocationError exposing .code for envelope rejections", async () => {
    invokeMock.mockRejectedValueOnce({
      code: "missing_credential",
      message: "No OpenAI key in keychain",
    });
    await expect(callCommand("typed_command")).rejects.toBeInstanceOf(
      CommandInvocationError,
    );
  });

  it("exposes code and message on the thrown typed error", async () => {
    invokeMock.mockRejectedValue({
      code: "network",
      message: "connection reset",
    });
    const error = (await callCommand("typed_command").catch(
      (e) => e,
    )) as CommandInvocationError;
    expect(error.code).toBe("network");
    expect(error.message).toBe("connection reset");
  });

  it("degrades safely: malformed objects reject unchanged", async () => {
    const malformed = { code: 42 };
    invokeMock.mockRejectedValueOnce(malformed);
    await expect(callCommand("weird_command")).rejects.toBe(malformed);
  });
});

describe("isCommandError guard", () => {
  it("accepts a well-formed envelope", () => {
    expect(isCommandError({ code: "conflict", message: "stale write" })).toBe(
      true,
    );
  });

  it("rejects strings, null, and malformed objects", () => {
    expect(isCommandError("nope")).toBe(false);
    expect(isCommandError(null)).toBe(false);
    expect(isCommandError({ code: 1, message: "x" })).toBe(false);
    expect(isCommandError({ message: "no code" })).toBe(false);
  });
});
