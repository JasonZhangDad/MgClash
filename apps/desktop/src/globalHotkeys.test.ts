import { beforeEach, describe, expect, it, vi } from "vitest";

const registerMock = vi.hoisted(() => vi.fn());
const unregisterAllMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/plugin-global-shortcut", () => ({
  register: registerMock,
  unregisterAll: unregisterAllMock,
}));

import {
  clearGlobalHotkeys,
  syncGlobalHotkeys,
  toGlobalShortcutChord,
} from "./globalHotkeys";

beforeEach(() => {
  registerMock.mockReset();
  unregisterAllMock.mockReset();
});

describe("toGlobalShortcutChord", () => {
  it("maps Ctrl to CommandOrControl and leaves other keys", () => {
    expect(toGlobalShortcutChord("Ctrl+Enter")).toBe("CommandOrControl+Enter");
    expect(toGlobalShortcutChord("Alt+[")).toBe("Alt+[");
    expect(toGlobalShortcutChord("Meta+]")).toBe("Super+]");
    expect(toGlobalShortcutChord("")).toBeNull();
    expect(toGlobalShortcutChord("   ")).toBeNull();
  });
});

describe("syncGlobalHotkeys", () => {
  it("registers distinct chords and reports success", async () => {
    unregisterAllMock.mockResolvedValue(undefined);
    registerMock.mockResolvedValue(undefined);
    const onAction = vi.fn();

    await expect(
      syncGlobalHotkeys(
        {
          connect: "Ctrl+Enter",
          previous: "Ctrl+[",
          next: "Ctrl+]",
        },
        onAction,
      ),
    ).resolves.toBe(true);

    expect(unregisterAllMock).toHaveBeenCalled();
    expect(registerMock).toHaveBeenCalledWith(
      ["CommandOrControl+Enter", "CommandOrControl+[", "CommandOrControl+]"],
      expect.any(Function),
    );

    const handler = registerMock.mock.calls[0]?.[1] as (event: {
      shortcut: string;
      state: string;
    }) => void;
    handler({ shortcut: "CommandOrControl+Enter", state: "Pressed" });
    expect(onAction).toHaveBeenCalledWith("connect");
    handler({ shortcut: "CommandOrControl+Enter", state: "Released" });
    expect(onAction).toHaveBeenCalledTimes(1);
  });

  it("returns false when the plugin is unavailable", async () => {
    unregisterAllMock.mockRejectedValue(new Error("not tauri"));
    await expect(
      syncGlobalHotkeys(
        { connect: "Ctrl+Enter", previous: "", next: "" },
        vi.fn(),
      ),
    ).resolves.toBe(false);
    expect(registerMock).not.toHaveBeenCalled();
  });

  it("clears previous bindings when every chord is empty", async () => {
    unregisterAllMock.mockResolvedValue(undefined);
    await expect(
      syncGlobalHotkeys({ connect: "", previous: "", next: "" }, vi.fn()),
    ).resolves.toBe(true);
    expect(registerMock).not.toHaveBeenCalled();
  });
});

describe("clearGlobalHotkeys", () => {
  it("swallows plugin errors", async () => {
    unregisterAllMock.mockRejectedValue(new Error("gone"));
    await expect(clearGlobalHotkeys()).resolves.toBeUndefined();
  });
});
