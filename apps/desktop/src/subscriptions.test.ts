import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  createSubscription,
  deleteSubscription,
  loadSubscriptions,
  refreshAllSubscriptions,
  refreshSubscription,
  updateSubscription,
} from "./subscriptions";

describe("subscription commands", () => {
  beforeEach(() => invokeMock.mockReset());

  it("lists and creates subscriptions", async () => {
    await loadSubscriptions();
    await createSubscription({
      autoUpdate: true,
      excludeKeywords: "",
      includeKeywords: "",
      name: "Primary",
      updateIntervalMinutes: 60,
      url: "https://example.com/list?token=secret",
      userAgent: null,
      subconverterUrl: null,
    });

    expect(invokeMock).toHaveBeenNthCalledWith(1, "subscription_list");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "subscription_create", {
      autoUpdate: true,
      excludeKeywords: "",
      includeKeywords: "",
      name: "Primary",
      updateIntervalMinutes: 60,
      url: "https://example.com/list?token=secret",
      userAgent: null,
      subconverterUrl: null,
    });
  });

  it("updates, refreshes, and deletes subscriptions", async () => {
    const id = "018f78b5-2cd0-7000-a9a6-3bccf60951e8";
    await updateSubscription({
      autoUpdate: false,
      enabled: true,
      excludeKeywords: "game",
      id,
      includeKeywords: "HK|TW",
      name: "Edited",
      updateIntervalMinutes: 120,
      url: null,
      userAgent: "MgClash-Test",
      subconverterUrl: null,
    });
    await refreshSubscription(id);
    await refreshAllSubscriptions();
    await deleteSubscription(id);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "subscription_update", {
      autoUpdate: false,
      enabled: true,
      excludeKeywords: "game",
      id,
      includeKeywords: "HK|TW",
      name: "Edited",
      updateIntervalMinutes: 120,
      url: null,
      userAgent: "MgClash-Test",
      subconverterUrl: null,
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "subscription_refresh", {
      id,
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "subscription_refresh_all");
    expect(invokeMock).toHaveBeenNthCalledWith(4, "subscription_delete", {
      id,
    });
  });
});
