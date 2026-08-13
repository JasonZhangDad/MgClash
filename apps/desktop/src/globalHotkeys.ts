import {
  register,
  unregisterAll,
  type ShortcutEvent,
} from "@tauri-apps/plugin-global-shortcut";

export type GlobalHotkeyAction = "connect" | "previous" | "next";

export interface GlobalHotkeyBindings {
  connect: string;
  next: string;
  previous: string;
}

/**
 * Maps a settings chord (`Ctrl+Enter`) to the global-shortcut plugin form.
 *
 * `Ctrl` becomes `CommandOrControl` so macOS users get ⌘ and Windows/Linux
 * keep Control — the same primary-modifier convention Tauri documents.
 * Empty bindings disable that action.
 */
export function toGlobalShortcutChord(binding: string): string | null {
  const parts = binding
    .split("+")
    .map((part) => part.trim())
    .filter((part) => part !== "");
  if (parts.length === 0) {
    return null;
  }
  return parts
    .map((part) => {
      if (part === "Ctrl") {
        return "CommandOrControl";
      }
      if (part === "Meta") {
        return "Super";
      }
      return part;
    })
    .join("+");
}

/**
 * Registers OS-level hotkeys for connect / previous / next.
 *
 * Returns `true` when at least one binding was registered with the plugin.
 * Returns `false` when every binding is empty or the plugin is unavailable
 * (browser tests, missing capability), so the caller can keep the in-window
 * keydown fallback.
 */
export async function syncGlobalHotkeys(
  bindings: GlobalHotkeyBindings,
  onAction: (action: GlobalHotkeyAction) => void,
): Promise<boolean> {
  try {
    await unregisterAll();
  } catch {
    return false;
  }

  const entries: Array<{ action: GlobalHotkeyAction; chord: string }> = [];
  const connect = toGlobalShortcutChord(bindings.connect);
  if (connect !== null) {
    entries.push({ action: "connect", chord: connect });
  }
  const previous = toGlobalShortcutChord(bindings.previous);
  if (previous !== null) {
    entries.push({ action: "previous", chord: previous });
  }
  const next = toGlobalShortcutChord(bindings.next);
  if (next !== null) {
    entries.push({ action: "next", chord: next });
  }
  if (entries.length === 0) {
    return true;
  }

  // Deduplicate identical chords so one physical key never fires two actions.
  const byChord = new Map<string, GlobalHotkeyAction>();
  for (const entry of entries) {
    byChord.set(entry.chord, entry.action);
  }

  try {
    await register([...byChord.keys()], (event: ShortcutEvent) => {
      if (event.state !== "Pressed") {
        return;
      }
      const action = byChord.get(event.shortcut);
      if (action !== undefined) {
        onAction(action);
      }
    });
    return true;
  } catch {
    return false;
  }
}

/** Clears every global shortcut this app registered. */
export async function clearGlobalHotkeys(): Promise<void> {
  try {
    await unregisterAll();
  } catch {
    // Outside Tauri there is nothing to clear.
  }
}
