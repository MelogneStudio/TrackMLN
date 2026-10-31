import { invoke } from "@tauri-apps/api/core";

/** Thin invoke wrapper: rejects with a plain string message. */
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(typeof error === "string" ? error : error instanceof Error ? error.message : String(error));
  }
}

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
