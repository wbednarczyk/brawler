import { invoke, type InvokeArgs } from "@tauri-apps/api/core";

export function callCommand<T>(command: string, args?: InvokeArgs) {
  if (args === undefined) {
    return invoke<T>(command);
  }

  return invoke<T>(command, args);
}
