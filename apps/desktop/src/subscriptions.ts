import { invoke } from "@tauri-apps/api/core";

export interface SubscriptionSummary {
  autoUpdate: boolean;
  enabled: boolean;
  id: string;
  lastUpdatedAt: number | null;
  name: string;
  nodeCount: number;
  updateIntervalMinutes: number;
}

export interface CreateSubscriptionInput {
  autoUpdate: boolean;
  name: string;
  updateIntervalMinutes: number;
  url: string;
}

export interface UpdateSubscriptionInput {
  autoUpdate: boolean;
  enabled: boolean;
  id: string;
  name: string;
  updateIntervalMinutes: number;
  url: string | null;
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

export function deleteSubscription(id: string): Promise<void> {
  return invoke<void>("subscription_delete", { id });
}
