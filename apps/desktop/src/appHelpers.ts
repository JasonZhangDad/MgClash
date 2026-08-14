import type { PlatformSummary } from "./platform";
import type { RouteRuleKind, RouteSettings } from "./session";
import { isCommandError } from "./session";

export const REFRESH_INTERVAL_MS = 3_000;
export const TRAFFIC_REFRESH_INTERVAL_MS = 1_000;
/// Only polled while the connection tab is open, so it can be brisk.
export const CONNECTIONS_REFRESH_INTERVAL_MS = 2_000;

export const UNSIGNED_NOTICE_KEY = "mgclash.unsignedNoticeDismissed";
export const URL_TEST_ADDRESS_KEY = "mgclash.urlTestAddress";
export const DEFAULT_URL_TEST_ADDRESS = "https://www.gstatic.com/generate_204";
export const DEFAULT_SPEED_TEST_URL =
  "https://speed.cloudflare.com/__down?bytes=10000000";
export const THEME_KEY = "mgclash.theme";
export const LAYOUT_KEY = "mgclash.mainLayout";

export type DialogId =
  | "subscriptions"
  | "dns"
  | "routing"
  | "geo"
  | "import"
  | "create"
  | "settings"
  | "about"
  | null;

export type MainLayout = "horizontal" | "vertical" | "tab";
export type MainTab = "profiles" | "proxies" | "connections" | "msg";
export type ThemeMode = "light" | "dark";

export const TUN_LABEL: Record<PlatformSummary["tunAvailability"], string> = {
  requiresElevation: "需要管理员权限",
  unavailableInUnsignedBuild: "未签名版本不可用",
};

export const TUN_NOTICE: Record<PlatformSummary["tunAvailability"], string> = {
  requiresElevation: "TUN 需要管理员权限才能启用。",
  unavailableInUnsignedBuild:
    "TUN 在未签名版本中不可用：它需要 Apple 签发的 Network Extension entitlement。本机可用的是本地 HTTP/SOCKS 代理与系统代理。",
};

export const ROUTE_KIND_LABEL: Record<RouteRuleKind, string> = {
  domain: "域名",
  domainSuffix: "域名后缀",
  domainKeyword: "域名关键字",
  ipCidr: "IPv4 CIDR",
  ipCidr6: "IPv6 CIDR",
  geoIp: "GeoIP",
  geoSite: "GeoSite",
  port: "端口",
  network: "网络",
  processName: "进程名",
  processPath: "进程路径",
};

export function isGeoRule(kind: RouteRuleKind): boolean {
  return kind === "geoIp" || kind === "geoSite";
}

export function runtimeOrderedRoute(settings: RouteSettings): RouteSettings {
  return {
    ...settings,
    rules: [
      ...settings.rules.filter((rule) => !isGeoRule(rule.kind)),
      ...settings.rules.filter((rule) => isGeoRule(rule.kind)),
    ],
  };
}

export function noticeWasDismissed(): boolean {
  try {
    return localStorage.getItem(UNSIGNED_NOTICE_KEY) === "1";
  } catch {
    return false;
  }
}

export function savedUrlTestAddress(): string {
  try {
    return localStorage.getItem(URL_TEST_ADDRESS_KEY)?.trim() || DEFAULT_URL_TEST_ADDRESS;
  } catch {
    return DEFAULT_URL_TEST_ADDRESS;
  }
}

export function savedTheme(): ThemeMode {
  try {
    return localStorage.getItem(THEME_KEY) === "dark" ? "dark" : "light";
  } catch {
    return "light";
  }
}

export function savedLayout(): MainLayout {
  try {
    const value = localStorage.getItem(LAYOUT_KEY);
    if (value === "vertical" || value === "tab" || value === "horizontal") {
      return value;
    }
  } catch {
    // fall through
  }
  return "horizontal";
}

const REGION_FLAGS: Record<string, string> = {
  AU: "🇦🇺",
  CA: "🇨🇦",
  CN: "🇨🇳",
  DE: "🇩🇪",
  FR: "🇫🇷",
  GB: "🇬🇧",
  HK: "🇭🇰",
  IN: "🇮🇳",
  JP: "🇯🇵",
  KR: "🇰🇷",
  NL: "🇳🇱",
  RU: "🇷🇺",
  SG: "🇸🇬",
  TW: "🇹🇼",
  UK: "🇬🇧",
  US: "🇺🇸",
};

export function regionFlag(name: string): string {
  const token = name.match(/\b([A-Z]{2})\b/) ?? name.match(/^([A-Z]{2})[-_\s]/);
  if (token !== null && REGION_FLAGS[token[1]] !== undefined) {
    return REGION_FLAGS[token[1]];
  }
  return "🌐";
}

export function latencyQuality(ms: number): "good" | "ok" | "bad" {
  if (ms < 100) {
    return "good";
  }
  if (ms < 200) {
    return "ok";
  }
  return "bad";
}

/** Stable ascending latency order; untested / failed (`null`) sort last. */
export function sortNodeIdsByLatency(
  nodes: readonly { id: string; latencyMs: number | null }[],
): string[] {
  return [...nodes]
    .sort((left, right) => {
      if (left.latencyMs === null && right.latencyMs === null) {
        return 0;
      }
      if (left.latencyMs === null) {
        return 1;
      }
      if (right.latencyMs === null) {
        return -1;
      }
      return left.latencyMs - right.latencyMs;
    })
    .map((node) => node.id);
}

/** Picks the enabled node with the lowest successful latency, if any. */
export function lowestLatencyNodeId(
  nodes: readonly {
    enabled: boolean;
    id: string;
    latencyMs: number | null;
  }[],
): string | null {
  let bestId: string | null = null;
  let bestMs = Number.POSITIVE_INFINITY;
  for (const node of nodes) {
    if (!node.enabled || node.latencyMs === null) {
      continue;
    }
    if (node.latencyMs < bestMs) {
      bestMs = node.latencyMs;
      bestId = node.id;
    }
  }
  return bestId;
}

export function formatClock(timestampMs: number): string {
  const date = new Date(timestampMs);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

export interface NodeMenuPosition {
  nodeId: string;
  x: number;
  y: number;
}

export function formatRate(bytesPerSecond: number): string {
  const units = ["B/s", "KB/s", "MB/s", "GB/s"];
  let value = bytesPerSecond;
  let unit = 0;
  while (value >= 1_024 && unit < units.length - 1) {
    value /= 1_024;
    unit += 1;
  }
  return unit === 0 ? `${value} ${units[unit]}` : `${value.toFixed(1)} ${units[unit]}`;
}

export function formatBytes(bytes: number): string {
  return formatRate(bytes).replace("/s", "");
}

/** Whether a keyboard event matches a stored chord like `Ctrl+Enter`. */
export function matchesHotkey(
  event: Pick<
    KeyboardEvent,
    "key" | "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey"
  >,
  binding: string,
): boolean {
  const parts = binding
    .split("+")
    .map((part) => part.trim())
    .filter((part) => part !== "");
  if (parts.length === 0) {
    return false;
  }
  const wantCtrl = parts.some((part) => part === "Ctrl");
  const wantAlt = parts.some((part) => part === "Alt");
  const wantShift = parts.some((part) => part === "Shift");
  const wantMeta = parts.some((part) => part === "Meta");
  if (
    event.ctrlKey !== wantCtrl ||
    event.altKey !== wantAlt ||
    event.shiftKey !== wantShift ||
    event.metaKey !== wantMeta
  ) {
    return false;
  }
  const key = parts[parts.length - 1] ?? "";
  if (key === "") {
    return false;
  }
  if (key === "Enter") {
    return event.key === "Enter";
  }
  if (key.length === 1) {
    return event.key.toLowerCase() === key.toLowerCase();
  }
  return event.key === key || event.code === key;
}

export function describeFailure(error: unknown): string {
  if (isCommandError(error)) {
    return error.message;
  }
  return error instanceof Error ? error.message : String(error);
}
