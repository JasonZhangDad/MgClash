import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { loadPlatformSummary } from "./platform";

describe("loadPlatformSummary", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("loads the platform summary through the Rust command", async () => {
    const summary = {
      artifactIdentifier: "macos-x86_64",
      tunAvailability: "unavailableInUnsignedBuild",
    };
    invokeMock.mockResolvedValue(summary);

    await expect(loadPlatformSummary()).resolves.toEqual(summary);
    expect(invokeMock).toHaveBeenCalledOnce();
    expect(invokeMock).toHaveBeenCalledWith("platform_summary");
  });

  it("surfaces command failures", async () => {
    const error = new Error("command failed");
    invokeMock.mockRejectedValue(error);

    await expect(loadPlatformSummary()).rejects.toBe(error);
  });
});
