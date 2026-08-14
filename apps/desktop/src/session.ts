import { invoke } from "@tauri-apps/api/core";

export type ProxyProtocol =
  | "anytls"
  | "custom"
  | "hysteria2"
  | "http"
  | "naive"
  | "shadowsocks"
  | "socks"
  | "trojan"
  | "tuic"
  | "vless"
  | "vmess"
  | "wireguard";

export interface NodeSummary {
  deletable: boolean;
  enabled: boolean;
  frontNodeId: string | null;
  groupId: string | null;
  /// The stream transport; Hysteria2/TUIC report their own QUIC transport,
  /// WireGuard its own tunnel, AnyTLS its TLS session, and Naive its HTTP/2 or
  /// QUIC tunnel.
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

export type NodeGroupStrategy = "select" | "urlTest" | "fallback" | "loadBalance";

export interface NodeGroupSummary {
  id: string;
  name: string;
  strategy: NodeGroupStrategy;
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

/** Four System Proxy choices aligned with v2rayN, including PAC. */

/** The language the window renders in. */
export type Locale =
  | "en"
  | "zh-Hans"
  | "zh-Hant"
  | "de"
  | "fr"
  | "es"
  | "it"
  | "ru"
  | "ja"
  | "ko";

export type SystemProxyMode = "managed" | "pac" | "cleared" | "unchanged";

export interface AppSettings {
  closeToTray: boolean;
  connectOnLaunch: boolean;
  corePreference: CorePreference;
  launchAtLogin: boolean;
  tunEnabled: boolean;
  logLevel: LogLevel;
  systemProxyMode: SystemProxyMode;
  locale: Locale;
  socksPort: number;
  httpPort: number;
  clashApiPort: number;
  muxEnabled: boolean;
  fragmentEnabled: boolean;
  finalFragmentEnabled: boolean;
  udpNoiseEnabled: boolean;
  autoSelectLowestLatency: boolean;
  urlTestAddress: string;
  urlTestIntervalSeconds: number;
  urlTestToleranceMs: number;
  allowLan: boolean;
  speedTestUrl: string;
  inboundUdpEnabled: boolean;
  defAllowInsecure: boolean;
  defFingerprint: string;
  hotkeyConnect: string;
  hotkeyPrevious: string;
  hotkeyNext: string;
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

export type TuicCongestionControl = "cubic" | "new_reno" | "bbr";

export type TuicUdpRelayMode = "native" | "quic";

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
    }
  | {
      congestionControl: TuicCongestionControl | null;
      password: string | null;
      protocol: "tuic";
      udpOverStream: boolean;
      udpRelayMode: TuicUdpRelayMode | null;
      uuid: string;
      zeroRttHandshake: boolean;
    }
  | {
      password: string | null;
      protocol: "socks";
      username: string | null;
    }
  | {
      password: string | null;
      protocol: "http";
      username: string | null;
    }
  | {
      localAddress: string[];
      mtu: number | null;
      peerPublicKey: string;
      preSharedKey: string | null;
      privateKey: string;
      protocol: "wireguard";
      reserved: [number, number, number] | null;
    }
  | { password: string; protocol: "anytls" }
  | {
      password: string | null;
      protocol: "naive";
      quic: boolean;
      quicCongestionControl: "bbr" | "bbr2" | "cubic" | "reno" | null;
      username: string | null;
    }
  | {
      core: "sing-box" | "xray";
      document: string;
      protocol: "custom";
    };

export type TransportDraft =
  | { type: "tcp" }
  | { host: string | null; path: string; type: "websocket" }
  | { host: string | null; path: string; type: "httpupgrade" }
  | {
      host: string | null;
      mode: "auto" | "packet-up" | "stream-up" | "stream-one";
      path: string;
      type: "xhttp";
    }
  | {
      authority: string | null;
      mode: "gun" | "multi" | "guna";
      serviceName: string;
      type: "grpc";
    }
  | {
      congestion: boolean;
      downlinkCapacity: number | null;
      headerType: string | null;
      mtu: number | null;
      seed: string | null;
      tti: number | null;
      type: "kcp";
      uplinkCapacity: number | null;
    };

export type TlsDraft =
  | {
      allowInsecure: boolean;
      alpn: string[];
      fingerprint: string | null;
      pinnedSha256: string | null;
      serverName: string | null;
      type: "tls";
    }
  | {
      alpn: string[];
      fingerprint: string | null;
      publicKey: string;
      serverName: string;
      shortId: string | null;
      spiderX: string | null;
      type: "reality";
    };

export interface ManualNodeDraft {
  credential: ManualCredentialDraft;
  name: string;
  port: number;
  server: string;
  tls: TlsDraft | null;
  transport: TransportDraft | null;
  udpEnabled: boolean;
  /** Optional Xray finalmask JSON (mask entry or `{tcp:[...]}`). */
  xrayFinalmaskJson?: string | null;
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

export interface RouteSchemeSummary {
  id: string;
  name: string;
}

export interface SessionStatus {
  connected: boolean;
  core: string;
  dns: DnsSettings;
  clashApiPort: number;
  httpPort: number;
  mode: RoutingMode;
  node: NodeSummary | null;
  route: RouteSettings;
  routeSchemeId: string;
  routeSchemes: RouteSchemeSummary[];
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

export type DnsTemplate = "simple" | "advanced";

export interface DnsSettings {
  bootstrap: string;
  dohPath: string;
  fakeIpEnabled: boolean;
  hosts: string;
  ipv6Enabled: boolean;
  mode: DnsMode;
  port: number;
  server: string;
  strategy: DnsStrategy;
  systemDomains: string[];
  template: DnsTemplate;
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
  | "network"
  | "processName"
  | "processPath";

export type RouteOutbound = "proxy" | "direct" | "block";

export interface RouteRuleSetting {
  enabled: boolean;
  kind: RouteRuleKind;
  outbound: RouteOutbound;
  value: string;
}

export type RuleProviderFormat = "binary" | "source";

/** A remote rule set the Core downloads and routes by. */
export interface RuleProviderSetting {
  enabled: boolean;
  format: RuleProviderFormat;
  name: string;
  outbound: RouteOutbound;
  url: string;
}

export interface RouteSettings {
  finalOutbound: RouteOutbound;
  providers: RuleProviderSetting[];
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

export function setRouteScheme(schemeId: string): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_set_route_scheme", { schemeId });
}

export function createRouteScheme(name: string): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_create_route_scheme", { name });
}

export function deleteRouteScheme(schemeId: string): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_delete_route_scheme", { schemeId });
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

export function loadNodeDraft(id: string): Promise<ManualNodeDraft> {
  return invoke<ManualNodeDraft>("session_node_draft", { id });
}

export function updateNode(
  id: string,
  draft: ManualNodeDraft,
): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_update_node", { id, draft });
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

export interface SpeedTestResult {
  bytesPerSecond: number | null;
  bytesRead: number | null;
  elapsedMs: number | null;
  id: string;
  status: "failed" | "success" | "timeout";
}

export function testDownloadSpeed(url: string): Promise<SpeedTestResult> {
  return invoke<SpeedTestResult>("session_speed_test", { url });
}

export function loadTraffic(): Promise<TrafficSnapshot> {
  return invoke<TrafficSnapshot>("session_traffic");
}

export function clearTraffic(): Promise<TrafficSnapshot> {
  return invoke<TrafficSnapshot>("session_clear_traffic");
}

/** One live connection as the Core's API reports it. */
export interface ConnectionSummary {
  chain: string;
  destination: string;
  downloadBytes: number;
  host: string;
  id: string;
  network: string;
  process: string;
  rule: string;
  start: string;
  uploadBytes: number;
}

export interface ConnectionSnapshot {
  connections: ConnectionSummary[];
  downloadTotalBytes: number;
  uploadTotalBytes: number;
}

/** What the rule-set table knows about one provider's downloaded copy. */
export interface RuleSetCacheEntry {
  bytes: number;
  cached: boolean;
  name: string;
  path: string;
  updatedAt: number | null;
}

export function loadRuleSets(): Promise<RuleSetCacheEntry[]> {
  return invoke<RuleSetCacheEntry[]>("rule_sets_status");
}

export function updateRuleSet(name: string): Promise<RuleSetCacheEntry[]> {
  return invoke<RuleSetCacheEntry[]>("rule_set_update", { name });
}

export function updateRuleSets(): Promise<RuleSetCacheEntry[]> {
  return invoke<RuleSetCacheEntry[]>("rule_sets_update_all");
}

export function loadConnections(): Promise<ConnectionSnapshot> {
  return invoke<ConnectionSnapshot>("session_connections");
}

export function closeConnection(id: string): Promise<void> {
  return invoke<void>("session_close_connection", { id });
}

export function closeConnections(): Promise<void> {
  return invoke<void>("session_close_connections");
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

/** Selects a node and reconnects when a session is already running. */
export function switchNode(id: string): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_switch_node", { id });
}

export function setNodeEnabled(
  id: string,
  enabled: boolean,
): Promise<NodeSummary[]> {
  return invoke<NodeSummary[]>("session_set_node_enabled", { enabled, id });
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

export function reorderNodes(ids: string[]): Promise<NodeSummary[]> {
  return invoke<NodeSummary[]>("session_reorder_nodes", { ids });
}

export function setNodeFront(
  id: string,
  frontId: string | null,
): Promise<NodeSummary[]> {
  return invoke<NodeSummary[]>("session_set_node_front", { frontId, id });
}

export function setNodeGroup(
  id: string,
  groupName: string | null,
): Promise<NodeSummary[]> {
  return invoke<NodeSummary[]>("session_set_node_group", { groupName, id });
}

export function setNodeGroupStrategy(
  id: string,
  strategy: NodeGroupStrategy,
): Promise<NodeGroupSummary[]> {
  return invoke<NodeGroupSummary[]>("session_set_node_group_strategy", {
    id,
    strategy,
  });
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

/** What a release check found. */
export interface UpdateCheck {
  current: string;
  latest: string;
  url: string;
  updateAvailable: boolean;
}

/** Asks GitHub whether a newer release exists. Only ever called from the menu:
 * the app contacts nothing on its own. */
export function checkUpdate(): Promise<UpdateCheck> {
  return invoke<UpdateCheck>("session_check_update");
}

export interface CoreVersionCheck {
  name: string;
  current: string;
  latest: string;
  url: string;
  updateAvailable: boolean;
  fromBinary: boolean;
}

export interface CoreUpdateCheck {
  singBox: CoreVersionCheck;
  xray: CoreVersionCheck;
  install: CoreInstallStatus;
}

/** Asks GitHub whether newer sing-box / Xray releases exist. Menu-only. */
/** Whether each Core can run right now, without downloading anything. */
export interface CoreReadiness {
  singBox: boolean;
  xray: boolean;
}

export function loadCoreReadiness(): Promise<CoreReadiness> {
  return invoke<CoreReadiness>("core_readiness");
}

export function checkCoreUpdate(): Promise<CoreUpdateCheck> {
  return invoke<CoreUpdateCheck>("core_check_update");
}

export interface InstalledCoreEntry {
  version: string;
  sha256: string;
  binary: string;
  previousVersion?: string;
}

export interface CoreInstallStatus {
  directory: string;
  singBox?: InstalledCoreEntry;
  xray?: InstalledCoreEntry;
}

/** Downloads and installs one Core from GitHub. Requires disconnect first. */
export function downloadCoreUpdate(core: "sing-box" | "xray"): Promise<CoreInstallStatus> {
  return invoke<CoreInstallStatus>("core_download_update", { core });
}

export interface GeoFileStatus {
  name: string;
  present: boolean;
  bytes: number;
  modifiedAt: number | null;
}

export interface GeoAssetsStatus {
  directory: string;
  geoip: GeoFileStatus;
  geosite: GeoFileStatus;
  assetEnvApplied: boolean;
}

export function loadGeoAssetsStatus(): Promise<GeoAssetsStatus> {
  return invoke<GeoAssetsStatus>("geo_assets_status");
}

export function updateGeoAssets(): Promise<GeoAssetsStatus> {
  return invoke<GeoAssetsStatus>("geo_assets_update");
}

/** Reads a sharing link out of a QR code image the user picked. */
export function readQrCode(image: Uint8Array): Promise<string> {
  return invoke<string>("session_read_qr_code", { image: [...image] });
}

/** The node's sharing link as an SVG QR code. The code carries the credential,
 * so it is only ever drawn on the user's own screen. */
export function nodeQrCode(id: string): Promise<string> {
  return invoke<string>("session_node_qr_code", { id });
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

export function exportPreferences(): Promise<string> {
  return invoke<string>("export_preferences");
}

export function importPreferences(path: string): Promise<AppSettings> {
  return invoke<AppSettings>("import_preferences", { path });
}

export interface ProfileImportResult {
  app: AppSettings;
  manualNodeCount: number;
  subscriptionCount: number;
}

export function exportProfile(): Promise<string> {
  return invoke<string>("export_profile");
}

export function importProfile(path: string): Promise<ProfileImportResult> {
  return invoke<ProfileImportResult>("import_profile", { path });
}
