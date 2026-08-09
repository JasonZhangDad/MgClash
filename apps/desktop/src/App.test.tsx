// @vitest-environment jsdom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const loadPlatformSummaryMock = vi.hoisted(() => vi.fn());

vi.mock("./platform", () => ({
  loadPlatformSummary: loadPlatformSummaryMock,
}));

import App from "./App";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

describe("App", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    loadPlatformSummaryMock.mockReset();
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it("shows the platform summary returned by Rust", async () => {
    loadPlatformSummaryMock.mockResolvedValue({
      artifactIdentifier: "macos-x86_64",
      tunAvailability: "unavailableInUnsignedBuild",
    });

    await act(async () => root.render(<App />));

    expect(container.textContent).toContain("macos-x86_64");
    expect(container.textContent).toContain("unavailableInUnsignedBuild");
  });

  it("shows command failures", async () => {
    loadPlatformSummaryMock.mockRejectedValue(new Error("command failed"));

    await act(async () => root.render(<App />));

    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "Error: command failed",
    );
  });
});
