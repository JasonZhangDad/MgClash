import type { PlatformSummary } from "./platform";
import type {
  NodeTraffic,
  RouteOutbound,
  RouteRuleKind,
  RouteSettings,
} from "./session";
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

/** The rule a connection suggests, or null when it names no host. */
export function ruleDraftFromConnection(
  host: string,
): { kind: RouteRuleKind; value: string } | null {
  const trimmed = host.trim();
  if (trimmed === "") {
    return null;
  }
  // The table renders `address:port`; the port belongs to the connection, not
  // to the rule. A bracketed IPv6 literal keeps its colons.
  const bracketed = /^\[(?<address>.+)\](?::\d+)?$/u.exec(trimmed);
  const withoutPort =
    bracketed?.groups?.address ??
    (trimmed.split(":").length === 2 ? trimmed.split(":")[0] : trimmed);
  if (/^\d{1,3}(?:\.\d{1,3}){3}$/u.test(withoutPort)) {
    return { kind: "ipCidr", value: `${withoutPort}/32` };
  }
  if (withoutPort.includes(":")) {
    return { kind: "ipCidr6", value: `${withoutPort}/128` };
  }
  return { kind: "domainSuffix", value: withoutPort };
}

/** What the rule-set table shows in its cache column. */
export function describeRuleSetCache(
  entries: {
    bytes: number;
    cached: boolean;
    name: string;
    updatedAt: number | null;
  }[],
  name: string,
  t: (text: string) => string,
): string {
  const entry = entries.find((candidate) => candidate.name === name);
  if (entry === undefined || !entry.cached) {
    return t("未缓存");
  }
  const size = formatBytes(entry.bytes);
  return entry.updatedAt === null
    ? size
    : `${size} · ${formatClock(entry.updatedAt * 1_000)}`;
}

/** One group's traffic, added up from the nodes that belong to it. */
export function groupTraffic(
  nodes: { groupId: string | null; id: string }[],
  traffic: Record<string, NodeTraffic>,
  groupId: string | null,
): NodeTraffic {
  return nodes
    .filter((node) => node.groupId === groupId)
    .reduce<NodeTraffic>(
      (total, node) => {
        const entry = traffic[node.id];
        if (entry === undefined) {
          return total;
        }
        return {
          todayUploadBytes: total.todayUploadBytes + entry.todayUploadBytes,
          todayDownloadBytes:
            total.todayDownloadBytes + entry.todayDownloadBytes,
          totalUploadBytes: total.totalUploadBytes + entry.totalUploadBytes,
          totalDownloadBytes:
            total.totalDownloadBytes + entry.totalDownloadBytes,
        };
      },
      {
        todayUploadBytes: 0,
        todayDownloadBytes: 0,
        totalUploadBytes: 0,
        totalDownloadBytes: 0,
      },
    );
}

/**
 * Live traffic per program, from the connections the Core is holding now.
 *
 * These are the bytes of the connections that are still open — the Core reports
 * no history, so a program that finished is not here.
 */
export function processTraffic(
  connections: {
    downloadBytes: number;
    process: string;
    uploadBytes: number;
  }[],
): { downloadBytes: number; name: string; uploadBytes: number }[] {
  const totals = new Map<
    string,
    { downloadBytes: number; uploadBytes: number }
  >();
  for (const connection of connections) {
    const name = connection.process === "" ? "—" : connection.process;
    const total = totals.get(name) ?? { downloadBytes: 0, uploadBytes: 0 };
    totals.set(name, {
      downloadBytes: total.downloadBytes + connection.downloadBytes,
      uploadBytes: total.uploadBytes + connection.uploadBytes,
    });
  }
  return [...totals.entries()]
    .map(([name, total]) => ({ ...total, name }))
    .sort(
      (left, right) =>
        right.downloadBytes +
        right.uploadBytes -
        (left.downloadBytes + left.uploadBytes),
    );
}

export const COLUMN_WIDTHS_KEY = "mgclash.columnWidths";
const MIN_COLUMN_WIDTH = 48;
const MAX_COLUMN_WIDTH = 1_200;

/** Keeps a dragged column usable: a zero-width column cannot be grabbed back. */
export function clampColumnWidth(width: number): number {
  return Math.min(MAX_COLUMN_WIDTH, Math.max(MIN_COLUMN_WIDTH, Math.round(width)));
}

/** The stored column widths, or an empty table when nothing usable is stored. */
export function loadColumnWidths(): Record<string, number> {
  try {
    const stored: unknown = JSON.parse(
      localStorage.getItem(COLUMN_WIDTHS_KEY) ?? "{}",
    );
    if (stored === null || typeof stored !== "object" || Array.isArray(stored)) {
      return {};
    }
    return Object.fromEntries(
      Object.entries(stored as Record<string, unknown>).filter(
        (entry): entry is [string, number] => typeof entry[1] === "number",
      ),
    );
  } catch {
    return {};
  }
}

export function saveColumnWidths(widths: Record<string, number>): void {
  try {
    localStorage.setItem(COLUMN_WIDTHS_KEY, JSON.stringify(widths));
  } catch {
    // A width that cannot be stored still applies to this launch.
  }
}

/** Which column the server table is ordered by, and which way. */
export type NodeSortColumn =
  | "name"
  | "protocol"
  | "server"
  | "latency";

export interface NodeSort {
  column: NodeSortColumn;
  direction: "asc" | "desc";
}

/**
 * The next state of the header the user just clicked.
 *
 * A third click drops back to the manual order the move-up/down actions
 * maintain, so sorting never permanently hides it.
 */
export function nextNodeSort(
  current: NodeSort | null,
  column: NodeSortColumn,
): NodeSort | null {
  if (current === null || current.column !== column) {
    return { column, direction: "asc" };
  }
  return current.direction === "asc" ? { column, direction: "desc" } : null;
}

/** Orders a copy of the rows; `null` keeps the stored order. */
export function sortNodes<
  T extends {
    latencyMs: number | null;
    name: string;
    protocol: string;
    server: string;
  },
>(nodes: T[], sort: NodeSort | null): T[] {
  if (sort === null) {
    return [...nodes];
  }
  const factor = sort.direction === "asc" ? 1 : -1;
  return [...nodes].sort((left, right) => {
    if (sort.column === "latency") {
      // A node nobody has measured sorts last either way: it is not fast, and
      // calling it the slowest would be a claim the app cannot make.
      if (left.latencyMs === null || right.latencyMs === null) {
        return (left.latencyMs === null ? 1 : 0) - (right.latencyMs === null ? 1 : 0);
      }
      return factor * (left.latencyMs - right.latencyMs);
    }
    const value = (node: T) =>
      sort.column === "name"
        ? node.name
        : sort.column === "protocol"
          ? node.protocol
          : node.server;
    return factor * value(left).localeCompare(value(right), undefined, { sensitivity: "base" });
  });
}

/** How each route outbound reads in the rule tables. */
export const ROUTE_OUTBOUND_LABEL: Record<RouteOutbound, string> = {
  block: "拦截",
  direct: "直连",
  proxy: "代理",
};

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
  protocol: "协议",
  inbound: "入站",
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
