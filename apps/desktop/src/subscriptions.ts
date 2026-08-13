import { invoke } from "@tauri-apps/api/core";

export interface SubscriptionSummary {
  autoUpdate: boolean;
  enabled: boolean;
  excludeKeywords: string;
  id: string;
  includeKeywords: string;
  lastError: string | null;
  lastUpdatedAt: number | null;
  name: string;
  nodeCount: number;
  updateIntervalMinutes: number;
  userAgent: string | null;
}

export interface CreateSubscriptionInput {
  autoUpdate: boolean;
  excludeKeywords: string;
  includeKeywords: string;
  name: string;
  updateIntervalMinutes: number;
  url: string;
  userAgent: string | null;
}

export interface UpdateSubscriptionInput {
  autoUpdate: boolean;
  enabled: boolean;
  excludeKeywords: string;
  id: string;
  includeKeywords: string;
  name: string;
  updateIntervalMinutes: number;
  url: string | null;
  userAgent: string | null;
}

export function loadSubscriptions(): Promise<SubscriptionSummary[]> {
  return invoke<SubscriptionSummary[]>("subscription_list");
}

export function createSubscription(
  input: CreateSubscriptionInput,
): Promise<SubscriptionSummary> {
  return invoke<SubscriptionSummary>("subscription_create", { ...input });
}

export function updateSubscription(
  input: UpdateSubscriptionInput,
): Promise<SubscriptionSummary> {
  return invoke<SubscriptionSummary>("subscription_update", { ...input });
}

export function refreshSubscription(
  id: string,
): Promise<SubscriptionSummary> {
  return invoke<SubscriptionSummary>("subscription_refresh", { id });
}

export function refreshAllSubscriptions(): Promise<SubscriptionSummary[]> {
  return invoke<SubscriptionSummary[]>("subscription_refresh_all");
}

export function deleteSubscription(id: string): Promise<void> {
  return invoke<void>("subscription_delete", { id });
}
