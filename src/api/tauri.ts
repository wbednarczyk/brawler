import { invoke, type InvokeArgs } from "@tauri-apps/api/core";

import type { CommandError } from "./generated/CommandError";
import type { CommandErrorCode } from "./generated/CommandErrorCode";

export type { CommandError, CommandErrorCode };

/**
 * Structural guard for the typed command error envelope (ADR 0070). A rejected
 * command payload is an envelope when it is an object carrying a string `code`
 * and a string `message`. Legacy commands reject with a bare string, and a
 * malformed object (e.g. numeric `code`) fails the guard and is left untouched.
 */
export function isCommandError(value: unknown): value is CommandError {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.code === "string" && typeof candidate.message === "string"
  );
}

/**
 * A rejected command whose payload matched the {@link CommandError} envelope,
 * surfaced as a real `Error` so callers can `catch` it uniformly while still
 * branching on the machine-readable {@link CommandErrorCode} via `.code`.
 */
export class CommandInvocationError extends Error {
  readonly code: CommandErrorCode;

  constructor(envelope: CommandError) {
    super(envelope.message);
    this.name = "CommandInvocationError";
    this.code = envelope.code;
  }
}

/**
 * Invoke a Tauri command. On rejection, an ADR 0070 envelope (`{code, message}`)
 * is re-thrown as a typed {@link CommandInvocationError}; every other rejection
 * (legacy bare strings, malformed objects) passes through unchanged so existing
 * callers keep working.
 */
export function callCommand<T>(
  command: string,
  args?: InvokeArgs,
): Promise<T> {
  const result =
    args === undefined ? invoke<T>(command) : invoke<T>(command, args);
  return result.catch((rejection: unknown) => {
    if (isCommandError(rejection)) {
      throw new CommandInvocationError(rejection);
    }
    throw rejection;
  });
}
