import { invoke } from "@tauri-apps/api/core";

export type ProxyProtocol =
  | "hysteria2"
  | "shadowsocks"
  | "trojan"
  | "vless"
  | "vmess";

export interface NodeSummary {
  deletable: boolean;
  groupId: string | null;
  /// The stream transport; Hysteria2 reports its own QUIC transport.
  transport: string;
  /// The TLS layer, or null for plaintext.
  tls: string | null;
  id: string;
  lastTestedAt: number | null;
  latencyMs: number | null;
  name: string;
  port: number;
  protocol: ProxyProtocol;
  server: string;
}

export interface NodeGroupSummary {
  id: string;
  name: string;
}

export interface NodeEdit {
  name: string;
  port: number;
  server: string;
}

export type NodeMoveDirection = "down" | "up";

export type LogLevel = "error" | "warn" | "info" | "debug" | "trace";

export type LogSource = "app" | "core";

export type CorePreference = "auto" | "sing-box" | "xray";

/** Three of v2rayN's four System Proxy choices; PAC has no counterpart yet. */
export type SystemProxyMode = "managed" | "pac" | "cleared" | "unchanged";

export interface AppSettings {
  closeToTray: boolean;
  connectOnLaunch: boolean;
  corePreference: CorePreference;
  launchAtLogin: boolean;
  tunEnabled: boolean;
  logLevel: LogLevel;
  systemProxyMode: SystemProxyMode;
}

export interface LogEntry {
  level: LogLevel;
  message: string;
  source: LogSource;
  timestampMs: number;
}

export interface BulkImportLineReport {
  /** Absent when the failure belongs to no single line. */
  line: number | null;
  message: string;
}

export interface BulkImportReport {
  duplicates: number;
  failures: BulkImportLineReport[];
  imported: number;
  status: SessionStatus;
}

export type VmessSecurity =
  | "Auto"
  | "Aes128Gcm"
  | "Chacha20Poly1305"
  | "None"
  | "Zero";

export type ObfuscationMethod = "Salamander" | "Gecko";

export type ManualCredentialDraft =
  | { flow: string | null; protocol: "vless"; userId: string }
  | {
      alterId: number;
      protocol: "vmess";
      security: VmessSecurity;
      userId: string;
    }
  | { password: string; protocol: "trojan" }
  | { method: string; password: string; protocol: "shadowsocks" }
  | {
      authentication: string | null;
      obfuscation: { method: ObfuscationMethod; password: string } | null;
      protocol: "hysteria2";
    };

export type TransportDraft =
  | { type: "tcp" }
  | { host: string | null; path: string; type: "websocket" }
  | {
      authority: string | null;
      mode: "gun" | "multi" | "guna";
      serviceName: string;
      type: "grpc";
    };

export type TlsDraft = {
  allowInsecure: boolean;
  alpn: string[];
  fingerprint: string | null;
  serverName: string | null;
  type: "tls";
};

export interface ManualNodeDraft {
  credential: ManualCredentialDraft;
  name: string;
  port: number;
  server: string;
  tls: TlsDraft | null;
  transport: TransportDraft | null;
  udpEnabled: boolean;
}

export type NodeTestStatus = "failed" | "success" | "timeout";

export interface NodeTestResult {
  id: string;
  latencyMs: number | null;
  status: NodeTestStatus;
}

export interface TrafficSnapshot {
  downloadBytesPerSecond: number;
  monthBytes: number;
  todayBytes: number;
  totalBytes: number;
  uploadBytesPerSecond: number;
}

export interface SessionStatus {
  connected: boolean;
  core: string;
  dns: DnsSettings;
  httpPort: number;
  mode: RoutingMode;
  node: NodeSummary | null;
  route: RouteSettings;
  socksPort: number;
  systemProxy: boolean;
  systemProxyMode: SystemProxyMode;
}

export type RoutingMode = "direct" | "global" | "rule";

export type DnsMode = "system" | "plainUdp" | "plainTcp" | "doh" | "dot";

export type DnsStrategy =
  | "preferIpv4"
  | "preferIpv6"
  | "ipv4Only"
  | "ipv6Only";

export interface DnsSettings {
  dohPath: string;
  fakeIpEnabled: boolean;
  ipv6Enabled: boolean;
  mode: DnsMode;
  port: number;
  server: string;
  strategy: DnsStrategy;
  systemDomains: string[];
}

export type RouteRuleKind =
  | "domain"
  | "domainSuffix"
  | "domainKeyword"
  | "ipCidr"
  | "ipCidr6"
  | "geoIp"
  | "geoSite"
  | "port"
  | "network";

export type RouteOutbound = "proxy" | "direct";

export interface RouteRuleSetting {
  enabled: boolean;
  kind: RouteRuleKind;
  outbound: RouteOutbound;
  value: string;
}

export interface RouteSettings {
  finalOutbound: RouteOutbound;
  rules: RouteRuleSetting[];
}

/** The shape every failed Tauri command returns. */
export interface CommandError {
  code: string;
  message: string;
}

export type SystemProxyStartupStatus = "clean" | "restoreRequired";

export function isCommandError(value: unknown): value is CommandError {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as CommandError).code === "string" &&
    typeof (value as CommandError).message === "string"
  );
}

export function loadSessionStatus(): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_status");
}

export function setRoutingMode(mode: RoutingMode): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_set_routing_mode", { mode });
}

export function setRouteSettings(settings: RouteSettings): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_set_route_settings", { settings });
}

export function setDnsSettings(settings: DnsSettings): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_set_dns_settings", { settings });
}

export function importNode(uri: string): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_import_node", { uri });
}

export function loadAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("app_settings");
}

export function saveAppSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("set_app_settings", { settings });
}

export function loadLogs(
  level: LogLevel,
  source: LogSource | null,
): Promise<LogEntry[]> {
  return invoke<LogEntry[]>("session_logs", { level, source });
}

export function clearLogs(): Promise<void> {
  return invoke<void>("session_clear_logs");
}

export function importNodes(content: string): Promise<BulkImportReport> {
  return invoke<BulkImportReport>("session_import_nodes", { content });
}

export function createNode(draft: ManualNodeDraft): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_create_node", { draft });
}

export function loadNodes(): Promise<NodeSummary[]> {
  return invoke<NodeSummary[]>("session_nodes");
}

export function loadNodeGroups(): Promise<NodeGroupSummary[]> {
  return invoke<NodeGroupSummary[]>("session_node_groups");
}

export function testNode(id: string): Promise<NodeTestResult> {
  return invoke<NodeTestResult>("session_test_node", { id });
}

export function testUrl(url: string): Promise<NodeTestResult> {
  return invoke<NodeTestResult>("session_url_test", { url });
}

export function loadTraffic(): Promise<TrafficSnapshot> {
  return invoke<TrafficSnapshot>("session_traffic");
}

export async function testAllNodes(
  ids: string[],
  onResult: (result: NodeTestResult) => void,
  isCancelled: () => boolean,
): Promise<void> {
  let nextIndex = 0;
  let firstFailure: unknown;
  let hasFailure = false;
  const workers = Array.from(
    { length: Math.min(8, ids.length) },
    async () => {
      while (!isCancelled() && !hasFailure) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= ids.length) {
          return;
        }
        try {
          onResult(await testNode(ids[index]));
        } catch (failure: unknown) {
          if (!hasFailure) {
            firstFailure = failure;
            hasFailure = true;
          }
        }
      }
    },
  );
  await Promise.all(workers);
  if (hasFailure) {
    throw firstFailure;
  }
}

export function selectNode(id: string): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_select_node", { id });
}

export function editNode(id: string, edit: NodeEdit): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_edit_node", { id, ...edit });
}

export function moveNode(
  id: string,
  direction: NodeMoveDirection,
): Promise<NodeSummary[]> {
  return invoke<NodeSummary[]>("session_move_node", { direction, id });
}

export function setNodeGroup(
  id: string,
  groupName: string | null,
): Promise<NodeSummary[]> {
  return invoke<NodeSummary[]>("session_set_node_group", { groupName, id });
}

export function deleteNode(id: string): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_delete_node", { id });
}

/** One node's own traffic, split by direction. */
export interface NodeTraffic {
  todayUploadBytes: number;
  todayDownloadBytes: number;
  totalUploadBytes: number;
  totalDownloadBytes: number;
}

/** Every node's counters, keyed by node id. */
export function loadNodeTraffic(): Promise<Record<string, NodeTraffic>> {
  return invoke<Record<string, NodeTraffic>>("session_node_traffic");
}

/** Duplicates a manual node, credential and all. Returns the new list. */
export function cloneNode(id: string): Promise<NodeSummary[]> {
  return invoke<NodeSummary[]>("session_clone_node", { id });
}

/** Deletes every node repeating one earlier in the list. Returns how many. */
export function removeDuplicateNodes(): Promise<number> {
  return invoke<number>("session_remove_duplicate_nodes");
}

/** The node's sharing URI. It carries the credential, so it is only ever
 * written to the clipboard the user asked for. */
export function exportNodeLink(id: string): Promise<string> {
  return invoke<string>("session_export_node_link", { id });
}

export function connectSession(): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_connect");
}

export function disconnectSession(): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_disconnect");
}

export function loadSystemProxyStartupStatus(): Promise<SystemProxyStartupStatus> {
  return invoke<SystemProxyStartupStatus>("system_proxy_startup_status");
}

export function recoverSystemProxy(): Promise<SystemProxyStartupStatus> {
  return invoke<SystemProxyStartupStatus>("system_proxy_recover");
}

export function dismissSystemProxyRecovery(): Promise<SystemProxyStartupStatus> {
  return invoke<SystemProxyStartupStatus>("system_proxy_dismiss");
}

/** Writes a redacted diagnostic bundle and resolves with its path. */
export function exportDiagnostics(): Promise<string> {
  return invoke<string>("export_diagnostics");
}
