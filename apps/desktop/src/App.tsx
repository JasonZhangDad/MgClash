import { useCallback, useEffect, useRef, useState } from "react";

import {
  buildManualNodeDraft,
  emptyManualNodeForm,
  SHADOWSOCKS_METHODS,
  usesStreamTransport,
  type GrpcMode,
  type ManualNodeForm,
  type TransportKind,
} from "./manualNode";
import { loadPlatformSummary, type PlatformSummary } from "./platform";
import {
  connectSession,
  createNode,
  cloneNode,
  deleteNode,
  nodeQrCode,
  readQrCode,
  exportNodeLink,
  removeDuplicateNodes,
  dismissSystemProxyRecovery,
  disconnectSession,
  editNode,
  exportDiagnostics,
  clearLogs,
  importNode,
  importNodes,
  isCommandError,
  loadAppSettings,
  loadLogs,
  saveAppSettings,
  loadNodeGroups,
  loadNodes,
  loadSessionStatus,
  loadSystemProxyStartupStatus,
  loadNodeTraffic,
  loadTraffic,
  moveNode,
  recoverSystemProxy,
  selectNode,
  setDnsSettings,
  setNodeGroup,
  setRouteSettings,
  setRoutingMode,
  testAllNodes,
  testNode,
  testUrl,
  type AppSettings,
  type BulkImportReport,
  type CorePreference,
  type LogEntry,
  type LogLevel,
  type LogSource,
  type NodeSummary,
  type NodeGroupSummary,
  type NodeTestResult,
  type ObfuscationMethod,
  type ProxyProtocol,
  type VmessSecurity,
  type DnsMode,
  type DnsSettings,
  type DnsStrategy,
  type RouteOutbound,
  type RouteRuleKind,
  type RouteSettings,
  type SessionStatus,
  type RoutingMode,
  type SystemProxyStartupStatus,
  type NodeTraffic,
  type SystemProxyMode,
  type TrafficSnapshot,
} from "./session";
import {
  createSubscription,
  deleteSubscription,
  loadSubscriptions,
  refreshAllSubscriptions,
  refreshSubscription,
  updateSubscription,
  type SubscriptionSummary,
} from "./subscriptions";

/**
 * Automatic network recovery can reconnect the session without the user acting,
 * so the dashboard re-reads the status instead of trusting its last command.
 */
const REFRESH_INTERVAL_MS = 3_000;
const TRAFFIC_REFRESH_INTERVAL_MS = 1_000;
const EMPTY_TRAFFIC: TrafficSnapshot = {
  downloadBytesPerSecond: 0,
  monthBytes: 0,
  todayBytes: 0,
  totalBytes: 0,
  uploadBytesPerSecond: 0,
};

const TUN_LABEL: Record<PlatformSummary["tunAvailability"], string> = {
  requiresElevation: "需要管理员权限",
  unavailableInUnsignedBuild: "未签名版本不可用",
};

/**
 * PRD V1.1 DoD 6 requires the unsigned-build risk to be visible on first
 * launch, not only on the download page.
 */
const UNSIGNED_NOTICE_KEY = "mgclash.unsignedNoticeDismissed";
const URL_TEST_ADDRESS_KEY = "mgclash.urlTestAddress";
const DEFAULT_URL_TEST_ADDRESS = "https://www.gstatic.com/generate_204";

const ROUTE_KIND_LABEL: Record<RouteRuleKind, string> = {
  domain: "域名",
  domainSuffix: "域名后缀",
  domainKeyword: "域名关键字",
  ipCidr: "IPv4 CIDR",
  ipCidr6: "IPv6 CIDR",
  geoIp: "GeoIP",
  geoSite: "GeoSite",
  port: "端口",
  network: "网络",
};

function isGeoRule(kind: RouteRuleKind): boolean {
  return kind === "geoIp" || kind === "geoSite";
}

function runtimeOrderedRoute(settings: RouteSettings): RouteSettings {
  return {
    ...settings,
    rules: [
      ...settings.rules.filter((rule) => !isGeoRule(rule.kind)),
      ...settings.rules.filter((rule) => isGeoRule(rule.kind)),
    ],
  };
}

const TUN_NOTICE: Record<PlatformSummary["tunAvailability"], string> = {
  requiresElevation: "TUN 需要管理员权限才能启用。",
  unavailableInUnsignedBuild:
    "TUN 在未签名版本中不可用：它需要 Apple 签发的 Network Extension entitlement。本机可用的是本地 HTTP/SOCKS 代理与系统代理。",
};

function noticeWasDismissed(): boolean {
  try {
    return localStorage.getItem(UNSIGNED_NOTICE_KEY) === "1";
  } catch {
    // A webview with storage disabled should still start; showing the notice
    // again is the safe direction to fail.
    return false;
  }
}

function savedUrlTestAddress(): string {
  try {
    return localStorage.getItem(URL_TEST_ADDRESS_KEY)?.trim() || DEFAULT_URL_TEST_ADDRESS;
  } catch {
    return DEFAULT_URL_TEST_ADDRESS;
  }
}

type PanelId =
  | "connection"
  | "nodes"
  | "subscriptions"
  | "routing"
  | "settings"
  | "logs";

/// The node table is the main view; everything else opens over it, which is how
/// v2rayN keeps the list in front while you configure.
const TABS: { id: PanelId; label: string }[] = [
  { id: "connection", label: "概览" },
  { id: "nodes", label: "添加节点" },
  { id: "subscriptions", label: "订阅分组" },
  { id: "routing", label: "路由设置" },
  { id: "settings", label: "参数设置" },
  { id: "logs", label: "诊断" },
];

/** Where the node menu opened, so it can be drawn at the pointer. */
interface NodeMenuPosition {
  nodeId: string;
  x: number;
  y: number;
}

function formatRate(bytesPerSecond: number): string {
  const units = ["B/s", "KB/s", "MB/s", "GB/s"];
  let value = bytesPerSecond;
  let unit = 0;
  while (value >= 1_024 && unit < units.length - 1) {
    value /= 1_024;
    unit += 1;
  }
  return unit === 0 ? `${value} ${units[unit]}` : `${value.toFixed(1)} ${units[unit]}`;
}

function formatBytes(bytes: number): string {
  return formatRate(bytes).replace("/s", "");
}

/** Command rejections are typed values from Rust, not `Error` instances. */
function describeFailure(error: unknown): string {
  if (isCommandError(error)) {
    return error.message;
  }
  return error instanceof Error ? error.message : String(error);
}

export default function App() {
  const [platform, setPlatform] = useState<PlatformSummary | null>(null);
  const [platformError, setPlatformError] = useState<string | null>(null);
  const [status, setStatus] = useState<SessionStatus | null>(null);
  const [dnsDraft, setDnsDraft] = useState<DnsSettings | null>(null);
  const [dnsDirty, setDnsDirty] = useState(false);
  const [routeDraft, setRouteDraft] = useState<RouteSettings | null>(null);
  const [routeDirty, setRouteDirty] = useState(false);
  const [routeRuleKind, setRouteRuleKind] = useState<RouteRuleKind>("domainSuffix");
  const [routeRuleValue, setRouteRuleValue] = useState("");
  const [routeRuleOutbound, setRouteRuleOutbound] =
    useState<RouteOutbound>("proxy");
  const [nodes, setNodes] = useState<NodeSummary[]>([]);
  const [nodeGroups, setNodeGroups] = useState<NodeGroupSummary[]>([]);
  const [nodeGroupFilter, setNodeGroupFilter] = useState("all");
  const [groupingNodeId, setGroupingNodeId] = useState<string | null>(null);
  const [nodeMenu, setNodeMenu] = useState<NodeMenuPosition | null>(null);
  const [checkedNodes, setCheckedNodes] = useState<Set<string>>(new Set());
  const [qrCode, setQrCode] = useState<{ name: string; svg: string } | null>(
    null,
  );
  const toggleCheckedNode = (id: string) =>
    setCheckedNodes((current) => {
      const next = new Set(current);
      if (!next.delete(id)) {
        next.add(id);
      }
      return next;
    });
  const openNodeMenu = (
    event: { preventDefault: () => void; clientX: number; clientY: number },
    nodeId: string,
  ) => {
    event.preventDefault();
    setNodeMenu({ nodeId, x: event.clientX, y: event.clientY });
  };
  const [nodeGroupName, setNodeGroupName] = useState("");
  const [editingNodeId, setEditingNodeId] = useState<string | null>(null);
  const [nodeName, setNodeName] = useState("");
  const [nodeServer, setNodeServer] = useState("");
  const [nodePort, setNodePort] = useState("");
  const [subscriptions, setSubscriptions] = useState<SubscriptionSummary[]>([]);
  const [uri, setUri] = useState("");
  const [createForm, setCreateForm] =
    useState<ManualNodeForm>(emptyManualNodeForm);
  const [panel, setPanel] = useState<PanelId | null>(null);
  const [bulkText, setBulkText] = useState("");
  const [bulkReport, setBulkReport] = useState<BulkImportReport | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [logLevel, setLogLevel] = useState<LogLevel>("info");
  const [logSource, setLogSource] = useState<LogSource | "all">("all");
  const [subscriptionName, setSubscriptionName] = useState("");
  const [subscriptionUrl, setSubscriptionUrl] = useState("");
  const [subscriptionInterval, setSubscriptionInterval] = useState("60");
  const [subscriptionAutoUpdate, setSubscriptionAutoUpdate] = useState(true);
  const [subscriptionEnabled, setSubscriptionEnabled] = useState(true);
  const [editingSubscriptionId, setEditingSubscriptionId] = useState<
    string | null
  >(null);
  const [error, setError] = useState<string | null>(null);
  const [exportedTo, setExportedTo] = useState<string | null>(null);
  const [systemProxyStartup, setSystemProxyStartup] =
    useState<SystemProxyStartupStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [traffic, setTraffic] = useState<TrafficSnapshot>(EMPTY_TRAFFIC);
  const [nodeTraffic, setNodeTraffic] = useState<Record<string, NodeTraffic>>({});
  const [urlTestAddress, setUrlTestAddress] = useState(savedUrlTestAddress);
  const [nodeTests, setNodeTests] = useState<
    Record<string, NodeTestResult | { status: "testing" }>
  >({});
  const [testingAllNodes, setTestingAllNodes] = useState(false);
  const cancelNodeTests = useRef(false);
  const [noticeDismissed, setNoticeDismissed] = useState(noticeWasDismissed);

  const nodeTestInProgress = Object.values(nodeTests).some(
    (result) => result.status === "testing",
  );

  useEffect(() => {
    if (nodeMenu === null) {
      return undefined;
    }
    const close = () => setNodeMenu(null);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        close();
      }
    };
    // Capture, so a click that also activates something still dismisses first.
    document.addEventListener("pointerdown", close, true);
    document.addEventListener("keydown", onKey);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("pointerdown", close, true);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", close);
    };
  }, [nodeMenu]);

  useEffect(() => {
    loadPlatformSummary().then(setPlatform, (failure: unknown) =>
      setPlatformError(describeFailure(failure)),
    );
    loadSessionStatus().then(setStatus, (failure: unknown) =>
      setError(describeFailure(failure)),
    );
    loadNodes().then(setNodes, (failure: unknown) =>
      setError(describeFailure(failure)),
    );
    loadNodeGroups().then(setNodeGroups, (failure: unknown) =>
      setError(describeFailure(failure)),
    );
    loadSubscriptions().then(setSubscriptions, (failure: unknown) =>
      setError(describeFailure(failure)),
    );
    loadSystemProxyStartupStatus().then(
      setSystemProxyStartup,
      (failure: unknown) => setError(describeFailure(failure)),
    );
  }, []);

  useEffect(() => {
    if (status !== null && !dnsDirty) {
      setDnsDraft(status.dns);
    }
  }, [dnsDirty, status]);

  useEffect(() => {
    if (status !== null && !routeDirty) {
      setRouteDraft(runtimeOrderedRoute(status.route));
    }
  }, [routeDirty, status]);

  useEffect(() => {
    // Skip while a command is in flight: it owns the status it is about to set.
    if (busy) {
      return undefined;
    }
    const timer = setInterval(() => {
      loadLogs(logLevel, logSource === "all" ? null : logSource).then(
        setLogs,
        (failure: unknown) => {
          console.warn("log refresh failed", failure);
        },
      );
      loadSessionStatus().then(setStatus, (failure: unknown) => {
        // A background refresh must not replace an error the user is reading.
        console.warn("session status refresh failed", failure);
      });
      loadSubscriptions().then(setSubscriptions, (failure: unknown) => {
        console.warn("subscription refresh failed", failure);
      });
    }, REFRESH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [busy, logLevel, logSource]);

  useEffect(() => {
    let active = true;
    let loading = false;
    const refresh = async () => {
      if (loading) {
        return;
      }
      loading = true;
      try {
        const perNode = await loadNodeTraffic().catch(() => null);
        if (perNode !== null) {
          setNodeTraffic(perNode);
        }
        const sample = await loadTraffic();
        if (active) {
          setTraffic(sample);
        }
      } catch (failure: unknown) {
        if (active) {
          setTraffic((current) => ({
            ...current,
            downloadBytesPerSecond: 0,
            uploadBytesPerSecond: 0,
          }));
          console.warn("traffic refresh failed", failure);
        }
      } finally {
        loading = false;
      }
    };
    void refresh();
    const timer = setInterval(() => void refresh(), TRAFFIC_REFRESH_INTERVAL_MS);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, []);

  const run = useCallback(async (command: () => Promise<SessionStatus>) => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setStatus(await command());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onExport = useCallback(async () => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setExportedTo(await exportDiagnostics());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onSaveDns = useCallback(async () => {
    if (dnsDraft === null) {
      return;
    }
    const server = dnsDraft.server.trim();
    if (dnsDraft.mode !== "system" && server === "") {
      setError("请填写 DNS 服务器");
      return;
    }
    if (
      dnsDraft.mode !== "system" &&
      (!Number.isInteger(dnsDraft.port) ||
        dnsDraft.port < 1 ||
        dnsDraft.port > 65_535)
    ) {
      setError("DNS 端口必须是 1 到 65535 的整数");
      return;
    }
    const settings = {
      ...dnsDraft,
      dohPath: dnsDraft.dohPath.trim(),
      server,
      systemDomains: dnsDraft.systemDomains
        .flatMap((value) => value.split(/[\n,]/))
        .map((value) => value.trim())
        .filter((value) => value !== ""),
    };

    setBusy(true);
    setError(null);
    try {
      const nextStatus = await setDnsSettings(settings);
      setStatus(nextStatus);
      setDnsDraft(nextStatus.dns);
      setDnsDirty(false);
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [dnsDraft]);

  const onAddRouteRule = useCallback(() => {
    const value = routeRuleValue.trim();
    if (routeDraft === null || value === "") {
      setError("请填写规则值");
      return;
    }
    const rule = {
      enabled: true,
      kind: routeRuleKind,
      outbound: routeRuleOutbound,
      value,
    };
    const rules = [...routeDraft.rules];
    if (isGeoRule(rule.kind)) {
      rules.push(rule);
    } else {
      const firstGeo = rules.findIndex((candidate) => isGeoRule(candidate.kind));
      rules.splice(firstGeo === -1 ? rules.length : firstGeo, 0, rule);
    }
    setRouteDraft({ ...routeDraft, rules });
    setRouteDirty(true);
    setRouteRuleValue("");
  }, [routeDraft, routeRuleKind, routeRuleOutbound, routeRuleValue]);

  const onMoveRouteRule = useCallback(
    (index: number, offset: -1 | 1) => {
      if (routeDraft === null) {
        return;
      }
      const target = index + offset;
      if (
        target < 0 ||
        target >= routeDraft.rules.length ||
        isGeoRule(routeDraft.rules[index].kind) !==
          isGeoRule(routeDraft.rules[target].kind)
      ) {
        return;
      }
      const rules = [...routeDraft.rules];
      [rules[index], rules[target]] = [rules[target], rules[index]];
      setRouteDraft({ ...routeDraft, rules });
      setRouteDirty(true);
    },
    [routeDraft],
  );

  const onSaveRoute = useCallback(async () => {
    if (routeDraft === null) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const nextStatus = await setRouteSettings(routeDraft);
      setStatus(nextStatus);
      setRouteDraft(runtimeOrderedRoute(nextStatus.route));
      setRouteDirty(false);
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [routeDraft]);

  const resolveSystemProxyStartup = useCallback(
    async (command: () => Promise<SystemProxyStartupStatus>) => {
      setBusy(true);
      setError(null);
      try {
        setSystemProxyStartup(await command());
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const dismissNotice = useCallback(() => {
    try {
      localStorage.setItem(UNSIGNED_NOTICE_KEY, "1");
    } catch {
      // Dismissal still applies to this launch even if it cannot be stored.
    }
    setNoticeDismissed(true);
  }, []);

  const onImport = useCallback(async () => {
    const sharingUri = uri.trim();
    if (sharingUri === "") {
      setError("请先粘贴分享链接");
      return;
    }
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setStatus(await importNode(sharingUri));
      setNodes(await loadNodes());
      setUri("");
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [uri]);

  const refreshLogs = useCallback(async () => {
    try {
      setLogs(await loadLogs(logLevel, logSource === "all" ? null : logSource));
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    }
  }, [logLevel, logSource]);

  // The saved level seeds the panel filter, so the log view opens where the
  // user left it rather than always at info.
  useEffect(() => {
    loadAppSettings().then(
      (loaded) => {
        setSettings(loaded);
        setLogLevel(loaded.logLevel);
      },
      (failure: unknown) => setError(describeFailure(failure)),
    );
  }, []);

  const onChangeSettings = useCallback(
    async (changes: Partial<AppSettings>) => {
      if (settings === null) {
        return;
      }
      const next = { ...settings, ...changes };
      setSettings(next);
      try {
        setSettings(await saveAppSettings(next));
        if (changes.logLevel !== undefined) {
          setLogLevel(changes.logLevel);
        }
      } catch (failure: unknown) {
        // Put the stored values back so the switches never claim a state the
        // app did not persist.
        setSettings(settings);
        setError(describeFailure(failure));
      }
    },
    [settings],
  );

  // Populate immediately on mount and whenever a filter changes; the shared
  // refresh interval only keeps an already-populated panel current.
  useEffect(() => {
    loadLogs(logLevel, logSource === "all" ? null : logSource).then(
      setLogs,
      (failure: unknown) => {
        console.warn("log load failed", failure);
      },
    );
  }, [logLevel, logSource]);

  const onClearLogs = useCallback(async () => {
    try {
      await clearLogs();
      setLogs([]);
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    }
  }, []);

  const runBulkImport = useCallback(async (content: string) => {
    if (content.trim() === "") {
      setError("请先粘贴节点链接或选择文件");
      return;
    }
    setBusy(true);
    setError(null);
    setExportedTo(null);
    setBulkReport(null);
    try {
      const report = await importNodes(content);
      setStatus(report.status);
      setNodes(await loadNodes());
      setBulkReport(report);
      if (report.imported > 0) {
        setBulkText("");
      }
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onImportFile = useCallback(
    async (file: File | undefined) => {
      if (!file) {
        return;
      }
      let content: string;
      try {
        content = await file.text();
      } catch {
        setError("无法读取所选文件");
        return;
      }
      await runBulkImport(content);
    },
    [runBulkImport],
  );

  /// Imports the node a QR code image holds.
  const onImportQrCode = useCallback(
    async (file: File | undefined) => {
      if (!file) {
        return;
      }
      setBusy(true);
      setError(null);
      setExportedTo(null);
      try {
        const image = new Uint8Array(await file.arrayBuffer());
        const link = await readQrCode(image);
        // Straight into the same bulk import a paste uses, so a scanned link
        // gets the same validation and the same report.
        await runBulkImport(link);
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [runBulkImport],
  );

  const updateCreateForm = useCallback(
    (changes: Partial<ManualNodeForm>) => {
      setCreateForm((current) => ({ ...current, ...changes }));
    },
    [],
  );

  const onCreateNode = useCallback(async () => {
    const result = buildManualNodeDraft(createForm);
    if ("error" in result) {
      setError(result.error);
      return;
    }
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setStatus(await createNode(result.draft));
      setNodes(await loadNodes());
      setCreateForm(emptyManualNodeForm);
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [createForm]);

  const onDeleteNode = useCallback(async (id: string) => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setStatus(await deleteNode(id));
      setNodes(await loadNodes());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onCloneNode = useCallback(async (id: string) => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setNodes(await cloneNode(id));
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onRemoveDuplicateNodes = useCallback(async () => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      const removed = await removeDuplicateNodes();
      setNodes(await loadNodes());
      setExportedTo(
        removed === 0 ? "没有重复节点" : `已移除 ${removed} 个重复节点`,
      );
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  /// Removes several nodes, reporting how many could not be removed rather
  /// than stopping at the first failure.
  const onDeleteNodes = useCallback(async (ids: string[]) => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    let failed = 0;
    try {
      for (const id of ids) {
        try {
          setStatus(await deleteNode(id));
        } catch {
          failed += 1;
        }
      }
      setNodes(await loadNodes());
      setCheckedNodes(new Set());
      if (failed > 0) {
        setError(`${failed} 个节点未能移除`);
      }
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  /// Copies several nodes as one link per line, which is what a bulk import
  /// reads back.
  const onExportNodeLinks = useCallback(async (ids: string[]) => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      const links: string[] = [];
      let failed = 0;
      for (const id of ids) {
        try {
          links.push(await exportNodeLink(id));
        } catch {
          failed += 1;
        }
      }
      await navigator.clipboard.writeText(links.join("\n"));
      setExportedTo(
        failed === 0
          ? `已复制 ${links.length} 条分享链接`
          : `已复制 ${links.length} 条，${failed} 条无法导出`,
      );
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onShowNodeQrCode = useCallback(async (candidate: NodeSummary) => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setQrCode({ name: candidate.name, svg: await nodeQrCode(candidate.id) });
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onExportNodeLink = useCallback(async (id: string) => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      const link = await exportNodeLink(id);
      // The link carries the credential, so it goes to the clipboard and is
      // never rendered: showing it would leave the secret on screen.
      await navigator.clipboard.writeText(link);
      setExportedTo("分享链接已复制到剪贴板");
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const resetNodeForm = useCallback(() => {
    setEditingNodeId(null);
    setNodeName("");
    setNodeServer("");
    setNodePort("");
  }, []);

  const onEditNode = useCallback((candidate: NodeSummary) => {
    setEditingNodeId(candidate.id);
    setNodeName(candidate.name);
    setNodeServer(candidate.server);
    setNodePort(String(candidate.port));
  }, []);

  const onSaveNode = useCallback(async () => {
    if (editingNodeId === null) {
      return;
    }
    const name = nodeName.trim();
    const server = nodeServer.trim();
    const port = Number(nodePort);
    if (name === "" || server === "") {
      setError("请填写节点名称和服务器");
      return;
    }
    if (!Number.isInteger(port) || port < 1 || port > 65_535) {
      setError("节点端口必须是 1 到 65535 的整数");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      setStatus(await editNode(editingNodeId, { name, port, server }));
      setNodes(await loadNodes());
      resetNodeForm();
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [editingNodeId, nodeName, nodePort, nodeServer, resetNodeForm]);

  const onMoveNode = useCallback(async (id: string, direction: "down" | "up") => {
    setBusy(true);
    setError(null);
    try {
      setNodes(await moveNode(id, direction));
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const resetNodeGroupForm = useCallback(() => {
    setGroupingNodeId(null);
    setNodeGroupName("");
  }, []);

  const onGroupNode = useCallback(
    (candidate: NodeSummary) => {
      setGroupingNodeId(candidate.id);
      setNodeGroupName(
        nodeGroups.find((group) => group.id === candidate.groupId)?.name ?? "",
      );
    },
    [nodeGroups],
  );

  const onSaveNodeGroup = useCallback(async () => {
    if (groupingNodeId === null) {
      return;
    }
    const groupName = nodeGroupName.trim();
    setBusy(true);
    setError(null);
    try {
      setNodes(await setNodeGroup(groupingNodeId, groupName || null));
      setNodeGroups(await loadNodeGroups());
      resetNodeGroupForm();
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [groupingNodeId, nodeGroupName, resetNodeGroupForm]);

  const onTestNode = useCallback(async (id: string) => {
    setError(null);
    setNodeTests((current) => ({ ...current, [id]: { status: "testing" } }));
    try {
      const result = await testNode(id);
      setNodeTests((current) => ({ ...current, [id]: result }));
    } catch (failure: unknown) {
      setNodeTests((current) => {
        const next = { ...current };
        delete next[id];
        return next;
      });
      setError(describeFailure(failure));
    }
  }, []);

  const onTestAllNodes = useCallback(async () => {
    cancelNodeTests.current = false;
    setError(null);
    setTestingAllNodes(true);
    setNodeTests(
      Object.fromEntries(
        nodes.map((candidate) => [candidate.id, { status: "testing" }]),
      ),
    );
    try {
      await testAllNodes(
        nodes.map((candidate) => candidate.id),
        (result) =>
          setNodeTests((current) => ({ ...current, [result.id]: result })),
        () => cancelNodeTests.current,
      );
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setNodeTests((current) =>
        Object.fromEntries(
          Object.entries(current).filter(
            ([, result]) => result.status !== "testing",
          ),
        ),
      );
      setTestingAllNodes(false);
    }
  }, [nodes]);

  /// Latency-tests a subset, reusing the cancellable batch the toolbar drives.
  const onTestNodes = useCallback(async (ids: string[]) => {
    cancelNodeTests.current = false;
    setError(null);
    setTestingAllNodes(true);
    setNodeTests(Object.fromEntries(ids.map((id) => [id, { status: "testing" }])));
    try {
      await testAllNodes(
        ids,
        (result) =>
          setNodeTests((current) => ({ ...current, [result.id]: result })),
        () => cancelNodeTests.current,
      );
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setNodeTests((current) =>
        Object.fromEntries(
          Object.entries(current).filter(
            ([, result]) => result.status !== "testing",
          ),
        ),
      );
      setTestingAllNodes(false);
    }
  }, []);

  const onCancelNodeTests = useCallback(() => {
    cancelNodeTests.current = true;
  }, []);

  const onTestUrl = useCallback(async () => {
    const address = urlTestAddress.trim();
    const selectedNode = status?.node;
    if (address === "") {
      setError("请填写 URL 测试地址");
      return;
    }
    if (!status?.connected || selectedNode === null || selectedNode === undefined) {
      setError("请先连接节点");
      return;
    }

    setUrlTestAddress(address);
    setError(null);
    setNodeTests((current) => ({
      ...current,
      [selectedNode.id]: { status: "testing" },
    }));
    try {
      const result = await testUrl(address);
      try {
        localStorage.setItem(URL_TEST_ADDRESS_KEY, address);
      } catch {
        // The test can still run when webview storage is unavailable.
      }
      setNodeTests((current) => ({ ...current, [result.id]: result }));
    } catch (failure: unknown) {
      setNodeTests((current) => {
        const next = { ...current };
        delete next[selectedNode.id];
        return next;
      });
      setError(describeFailure(failure));
    }
  }, [status, urlTestAddress]);

  const resetSubscriptionForm = useCallback(() => {
    setEditingSubscriptionId(null);
    setSubscriptionName("");
    setSubscriptionUrl("");
    setSubscriptionInterval("60");
    setSubscriptionAutoUpdate(true);
    setSubscriptionEnabled(true);
  }, []);

  const syncNodes = useCallback(async () => {
    const [nextNodes, nextStatus] = await Promise.all([
      loadNodes(),
      loadSessionStatus(),
    ]);
    setNodes(nextNodes);
    setStatus(nextStatus);
  }, []);

  const onSaveSubscription = useCallback(async () => {
    const name = subscriptionName.trim();
    const url = subscriptionUrl.trim();
    const updateIntervalMinutes = Number(subscriptionInterval);
    if (name === "" || (editingSubscriptionId === null && url === "")) {
      setError("请填写订阅名称和地址");
      return;
    }
    if (!Number.isInteger(updateIntervalMinutes) || updateIntervalMinutes < 1) {
      setError("更新间隔必须是正整数");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      const saved =
        editingSubscriptionId === null
          ? await createSubscription({
              autoUpdate: subscriptionAutoUpdate,
              name,
              updateIntervalMinutes,
              url,
            })
          : await updateSubscription({
              autoUpdate: subscriptionAutoUpdate,
              enabled: subscriptionEnabled,
              id: editingSubscriptionId,
              name,
              updateIntervalMinutes,
              url: url === "" ? null : url,
            });
      setSubscriptions((current) => {
        const existing = current.findIndex((item) => item.id === saved.id);
        if (existing === -1) {
          return [...current, saved];
        }
        return current.map((item) => (item.id === saved.id ? saved : item));
      });
      await syncNodes();
      resetSubscriptionForm();
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [
    editingSubscriptionId,
    resetSubscriptionForm,
    subscriptionAutoUpdate,
    subscriptionEnabled,
    subscriptionInterval,
    subscriptionName,
    subscriptionUrl,
    syncNodes,
  ]);

  const onEditSubscription = useCallback((item: SubscriptionSummary) => {
    setEditingSubscriptionId(item.id);
    setSubscriptionName(item.name);
    setSubscriptionUrl("");
    setSubscriptionInterval(String(item.updateIntervalMinutes));
    setSubscriptionAutoUpdate(item.autoUpdate);
    setSubscriptionEnabled(item.enabled);
  }, []);

  const onRefreshSubscription = useCallback(async (id: string) => {
    setBusy(true);
    setError(null);
    try {
      const refreshed = await refreshSubscription(id);
      setSubscriptions((current) =>
        current.map((item) => (item.id === id ? refreshed : item)),
      );
      await syncNodes();
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [syncNodes]);

  const onRefreshAllSubscriptions = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setSubscriptions(await refreshAllSubscriptions());
      await syncNodes();
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [syncNodes]);

  const onDeleteSubscription = useCallback(
    async (id: string) => {
      setBusy(true);
      setError(null);
      try {
        await deleteSubscription(id);
        setSubscriptions((current) =>
          current.filter((item) => item.id !== id),
        );
        await syncNodes();
        if (editingSubscriptionId === id) {
          resetSubscriptionForm();
        }
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [editingSubscriptionId, resetSubscriptionForm, syncNodes],
  );

  const connected = status?.connected ?? false;
  const node = status?.node ?? null;
  const nodeGroupNames = new Map(
    nodeGroups.map((group) => [group.id, group.name]),
  );
  const visibleNodes = nodes.filter((candidate) => {
    if (nodeGroupFilter === "all") {
      return true;
    }
    if (nodeGroupFilter === "ungrouped") {
      return candidate.groupId === null;
    }
    return candidate.groupId === nodeGroupFilter;
  });

  return (
    <main className="app-shell">
      <header className="menubar">
        <span className="brand">MgClash</span>
        <nav aria-label="配置入口">
          {TABS.map((entry) => (
            <button
              key={entry.id}
              type="button"
              className="menu-item"
              onClick={() => setPanel(entry.id)}
            >
              {entry.label}
            </button>
          ))}
        </nav>
        <button
          type="button"
          className={connected ? "primary danger" : "primary"}
          disabled={busy || node === null || systemProxyStartup !== "clean"}
          onClick={() => void run(connected ? disconnectSession : connectSession)}
        >
          {connected ? "断开" : "连接"}
        </button>
      </header>

      <section className="content">
        {systemProxyStartup === "restoreRequired" && (
          <section className="notice" aria-label="系统代理恢复">
            <p>
              检测到上次异常退出留下的系统代理设置。恢复原设置可以避免系统继续指向已经停止的本地代理。
            </p>
            <div className="actions">
              <button
                type="button"
                disabled={busy}
                onClick={() =>
                  void resolveSystemProxyStartup(recoverSystemProxy)
                }
              >
                恢复原设置
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() =>
                  void resolveSystemProxyStartup(dismissSystemProxyRecovery)
                }
              >
                保留当前设置
              </button>
            </div>
          </section>
        )}

        {!noticeDismissed && (
          <div className="notice" role="note">
            <p>
              这是<strong>未签名</strong>版本：macOS Gatekeeper 与 Windows
              SmartScreen 会在首次打开时提示，需要你手动确认后才能运行。
            </p>
            <p>{platform ? TUN_NOTICE[platform.tunAvailability] : ""}</p>
            <div className="actions">
              <button type="button" onClick={dismissNotice}>
                我知道了
              </button>
            </div>
          </div>
        )}

        <div className="node-panel">
        <h2>节点</h2>

        <div className="url-test">
          <label>
            URL 测试地址
            <input
              aria-label="URL 测试地址"
              value={urlTestAddress}
              disabled={busy || nodeTestInProgress}
              onChange={(event) => setUrlTestAddress(event.target.value)}
            />
          </label>
          <button
            type="button"
            disabled={busy || nodeTestInProgress || !connected || node === null}
            onClick={() => void onTestUrl()}
          >
            URL 测试
          </button>
        </div>
        <div className="log-pane">
        <h2>日志</h2>

        <p className="hint">
          Core 输出在写入前已脱敏，凭据字段一律替换为 [REDACTED]。最多保留最近 2000 条。
        </p>

        <div className="log-controls">
          <label>
            级别
            <select
              aria-label="日志级别"
              value={logLevel}
              onChange={(event) => setLogLevel(event.target.value as LogLevel)}
            >
              <option value="error">error</option>
              <option value="warn">warn</option>
              <option value="info">info</option>
              <option value="debug">debug</option>
              <option value="trace">trace</option>
            </select>
          </label>
          <label>
            来源
            <select
              aria-label="日志来源"
              value={logSource}
              onChange={(event) =>
                setLogSource(event.target.value as LogSource | "all")
              }
            >
              <option value="all">全部</option>
              <option value="app">应用</option>
              <option value="core">Core</option>
            </select>
          </label>
          <button type="button" onClick={() => void refreshLogs()}>
            刷新日志
          </button>
          <button type="button" onClick={() => void onClearLogs()}>
            清空日志
          </button>
        </div>

        {logs.length === 0 ? (
          <p className="hint">暂无日志</p>
        ) : (
          <ul className="log-list" aria-label="日志列表">
            {logs.map((entry, index) => (
              <li
                key={`${entry.timestampMs}-${index}`}
                className={`log-entry log-${entry.level}`}
              >
                <span className="log-time">
                  {new Date(entry.timestampMs).toLocaleTimeString()}
                </span>
                <span className="log-source">
                  {entry.source === "core" ? "Core" : "应用"}
                </span>
                <span className="log-level">{entry.level}</span>
                <span className="log-message">{entry.message}</span>
              </li>
            ))}
          </ul>
        )}

        </div>

        <div className="actions">
          {testingAllNodes ? (
            <button type="button" onClick={onCancelNodeTests}>
              取消测速
            </button>
          ) : (
            <button
              type="button"
              disabled={busy || nodeTestInProgress || nodes.length === 0}
              onClick={() => void onTestAllNodes()}
            >
              全部测速
            </button>
          )}
        </div>

        <div className="node-group-filter">
          <label>
            分组
            <select
              aria-label="节点分组筛选"
              value={nodeGroupFilter}
              onChange={(event) => setNodeGroupFilter(event.target.value)}
            >
              <option value="all">全部</option>
              <option value="ungrouped">未分组</option>
              {nodeGroups.map((group) => (
                <option key={group.id} value={group.id}>
                  {group.name}
                </option>
              ))}
            </select>
          </label>
        </div>

        {nodes.length === 0 ? (
          <p className="hint">尚未导入节点</p>
        ) : visibleNodes.length === 0 ? (
          <p className="hint">当前分组没有节点</p>
        ) : (
          <table className="node-list" aria-label="节点列表">
            <thead>
              <tr>
                <th className="node-check">
                  <input
                    type="checkbox"
                    aria-label="全选节点"
                    checked={
                      visibleNodes.length > 0 &&
                      visibleNodes.every((item) => checkedNodes.has(item.id))
                    }
                    onChange={(event) =>
                      setCheckedNodes(
                        event.target.checked
                          ? new Set(visibleNodes.map((item) => item.id))
                          : new Set(),
                      )
                    }
                  />
                </th>
                <th>名称</th>
                <th>协议</th>
                <th>传输</th>
                <th>TLS</th>
                <th>分组</th>
                <th>服务器</th>
                <th>延迟</th>
                <th>速度</th>
                <th>今日上传</th>
                <th>今日下载</th>
                <th>总上传</th>
                <th>总下载</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {visibleNodes.map((candidate) => {
                const index = nodes.findIndex((node) => node.id === candidate.id);
                const selected = candidate.id === node?.id;
                const testResult = nodeTests[candidate.id];
                let latency =
                  candidate.latencyMs === null
                    ? candidate.lastTestedAt === null
                      ? "—"
                      : "失败"
                    : `${candidate.latencyMs} ms`;
                if (testResult?.status === "testing") {
                  latency = "测试中";
                } else if (testResult?.status === "success") {
                  latency = `${testResult.latencyMs} ms`;
                } else if (testResult?.status === "timeout") {
                  latency = "超时";
                } else if (testResult?.status === "failed") {
                  latency = "失败";
                }
                return (
                  <tr
                    key={candidate.id}
                    className={selected ? "active-node" : undefined}
                    onContextMenu={(event) => openNodeMenu(event, candidate.id)}
                    onDoubleClick={() => {
                      if (!busy && !connected && !selected && !nodeTestInProgress) {
                        void run(() => selectNode(candidate.id));
                      }
                    }}
                  >
                    <td className="node-check">
                      <input
                        type="checkbox"
                        aria-label={`选择 ${candidate.name}`}
                        checked={checkedNodes.has(candidate.id)}
                        onChange={() => toggleCheckedNode(candidate.id)}
                      />
                    </td>
                    <td>{candidate.name}</td>
                    <td>{candidate.protocol}</td>
                    <td>{candidate.transport}</td>
                    <td>{candidate.tls ?? "—"}</td>
                    <td>{
                      candidate.groupId === null
                        ? "未分组"
                        : (nodeGroupNames.get(candidate.groupId) ?? "未知分组")
                    }</td>
                    <td>{`${candidate.server}:${candidate.port}`}</td>
                    <td>{latency}</td>
                    <td>
                      {connected && selected
                        ? `${formatRate(traffic.downloadBytesPerSecond)}`
                        : "—"}
                    </td>
                    <td>{formatBytes(nodeTraffic[candidate.id]?.todayUploadBytes ?? 0)}</td>
                    <td>{formatBytes(nodeTraffic[candidate.id]?.todayDownloadBytes ?? 0)}</td>
                    <td>{formatBytes(nodeTraffic[candidate.id]?.totalUploadBytes ?? 0)}</td>
                    <td>{formatBytes(nodeTraffic[candidate.id]?.totalDownloadBytes ?? 0)}</td>
                    <td className="node-actions">
                      <button
                        type="button"
                        aria-label={`操作 ${candidate.name}`}
                        aria-haspopup="menu"
                        disabled={busy || nodeTestInProgress}
                        onClick={(event) => openNodeMenu(event, candidate.id)}
                      >
                        ⋯
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}

        {nodeMenu !== null &&
          (() => {
            const target = nodes.find((item) => item.id === nodeMenu.nodeId);
            if (target === undefined) {
              return null;
            }
            const index = nodes.findIndex((item) => item.id === target.id);
            const selected = target.id === node?.id;
            // Reordering acts on the whole list, so it cannot follow a filtered view.
            const reorderable = nodeGroupFilter === "all";
            const act = (run_: () => void) => () => {
              setNodeMenu(null);
              run_();
            };
            // Right-clicking inside the selection acts on all of it;
            // right-clicking outside is a fresh act, not an extension of it.
            // The selection is intersected with what the filter shows, so a
            // batch never touches a row the user cannot see.
            const visible = new Set(visibleNodes.map((item) => item.id));
            const batch = checkedNodes.has(target.id)
              ? [...checkedNodes].filter((id) => visible.has(id))
              : [target.id];
            const suffix = batch.length > 1 ? ` (${batch.length})` : "";
            return (
              <ul
                className="context-menu"
                role="menu"
                aria-label={`节点操作 ${target.name}`}
                style={{ left: nodeMenu.x, top: nodeMenu.y }}
              >
                <li>
                  <button
                    type="button"
                    role="menuitem"
                    disabled={busy || connected || selected}
                    onClick={act(() => void run(() => selectNode(target.id)))}
                  >
                    设为活动
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    role="menuitem"
                    disabled={busy}
                    onClick={act(() =>
                      batch.length > 1
                        ? void onTestNodes(batch)
                        : void onTestNode(target.id),
                    )}
                  >
                    {`测试延迟${suffix}`}
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    role="menuitem"
                    disabled={busy || connected || !target.deletable}
                    onClick={act(() => void onCloneNode(target.id))}
                  >
                    克隆所选
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    role="menuitem"
                    disabled={busy}
                    onClick={act(() =>
                      batch.length > 1
                        ? void onExportNodeLinks(batch)
                        : void onExportNodeLink(target.id),
                    )}
                  >
                    {`导出分享链接${suffix}`}
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    role="menuitem"
                    disabled={busy}
                    onClick={act(() => void onShowNodeQrCode(target))}
                  >
                    显示二维码
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    role="menuitem"
                    disabled={busy}
                    onClick={act(() => onGroupNode(target))}
                  >
                    设置分组
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    role="menuitem"
                    disabled={busy || !reorderable || index === 0}
                    onClick={act(() => void onMoveNode(target.id, "up"))}
                  >
                    上移
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    role="menuitem"
                    disabled={busy || !reorderable || index === nodes.length - 1}
                    onClick={act(() => void onMoveNode(target.id, "down"))}
                  >
                    下移
                  </button>
                </li>
                {target.deletable && (
                  <li>
                    <button
                      type="button"
                      role="menuitem"
                      disabled={busy || connected}
                      onClick={act(() => onEditNode(target))}
                    >
                      编辑
                    </button>
                  </li>
                )}
                <li>
                  <button
                    type="button"
                    role="menuitem"
                    className="danger"
                    disabled={busy || connected || !target.deletable}
                    onClick={act(() =>
                      batch.length > 1
                        ? void onDeleteNodes(batch)
                        : void onDeleteNode(target.id),
                    )}
                  >
                    {target.deletable ? `移除所选${suffix}` : "订阅节点不可移除"}
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    role="menuitem"
                    className="danger"
                    disabled={busy || connected}
                    onClick={act(() => void onRemoveDuplicateNodes())}
                  >
                    移除重复
                  </button>
                </li>
              </ul>
            );
          })()}

        {groupingNodeId !== null && (
          <div className="settings-form" aria-label="设置节点分组">
            <label>
              分组名称
              <input
                aria-label="节点分组"
                disabled={busy}
                list="node-group-options"
                value={nodeGroupName}
                onChange={(event) => setNodeGroupName(event.target.value)}
              />
              <datalist id="node-group-options">
                {nodeGroups.map((group) => (
                  <option key={group.id} value={group.name} />
                ))}
              </datalist>
            </label>
            <div className="actions">
              <button type="button" disabled={busy} onClick={() => void onSaveNodeGroup()}>
                保存分组
              </button>
              <button type="button" disabled={busy} onClick={resetNodeGroupForm}>
                取消
              </button>
            </div>
          </div>
        )}

        {editingNodeId !== null && (
          <div className="settings-form" aria-label="编辑节点">
            <label>
              名称
              <input
                aria-label="节点名称"
                disabled={busy || connected}
                value={nodeName}
                onChange={(event) => setNodeName(event.target.value)}
              />
            </label>
            <label>
              服务器
              <input
                aria-label="节点服务器"
                disabled={busy || connected}
                value={nodeServer}
                onChange={(event) => setNodeServer(event.target.value)}
              />
            </label>
            <label>
              端口
              <input
                aria-label="节点端口"
                disabled={busy || connected}
                min="1"
                max="65535"
                type="number"
                value={nodePort}
                onChange={(event) => setNodePort(event.target.value)}
              />
            </label>
            <div className="actions">
              <button type="button" disabled={busy || connected} onClick={() => void onSaveNode()}>
                保存节点
              </button>
              <button type="button" disabled={busy} onClick={resetNodeForm}>
                取消
              </button>
            </div>
          </div>
        )}

        </div>
        {qrCode !== null && (
          <div className="dialog-backdrop" onClick={() => setQrCode(null)}>
            <div
              className="dialog qr-dialog"
              role="dialog"
              aria-label={`二维码 ${qrCode.name}`}
              onClick={(event) => event.stopPropagation()}
            >
              <header className="dialog-head">
                <strong>{qrCode.name}</strong>
                <button type="button" onClick={() => setQrCode(null)}>
                  关闭
                </button>
              </header>
              {/* The markup comes from the Rust renderer, never from a node's
                  own fields, so there is nothing here a node name could inject. */}
              <div
                className="qr-code"
                dangerouslySetInnerHTML={{ __html: qrCode.svg }}
              />
              <p className="hint">
                扫描即导入该节点。二维码包含凭据，请勿分享给他人。
              </p>
            </div>
          </div>
        )}
        <div
          className="dialog-backdrop"
          hidden={panel === null}
          onClick={() => setPanel(null)}
        >
          <div
            className="dialog"
            role="dialog"
            aria-label="配置"
            onClick={(event) => event.stopPropagation()}
          >
            <header className="dialog-head">
              <div className="tabs">
                {TABS.map((entry) => (
                  <button
                    key={entry.id}
                    type="button"
                    className={panel === entry.id ? "tab active" : "tab"}
                    onClick={() => setPanel(entry.id)}
                  >
                    {entry.label}
                  </button>
                ))}
              </div>
              <button
                type="button"
                className="dialog-close"
                aria-label="关闭配置"
                onClick={() => setPanel(null)}
              >
                关闭
              </button>
            </header>
            <div className="dialog-body">
              <div className="tab-panel" hidden={panel !== "connection"}>
        <h2>连接</h2>

        <dl>
          <div>
            <dt>状态</dt>
            <dd className={connected ? "connected" : undefined}>
              {connected ? "已连接" : "未连接"}
            </dd>
          </div>
          <div>
            <dt>下载</dt>
            <dd aria-label="下载速率">
              {connected ? formatRate(traffic.downloadBytesPerSecond) : "—"}
            </dd>
          </div>
          <div>
            <dt>上传</dt>
            <dd aria-label="上传速率">
              {connected ? formatRate(traffic.uploadBytesPerSecond) : "—"}
            </dd>
          </div>
          <div>
            <dt>今日</dt>
            <dd aria-label="今日流量">{formatBytes(traffic.todayBytes)}</dd>
          </div>
          <div>
            <dt>本月</dt>
            <dd aria-label="本月流量">{formatBytes(traffic.monthBytes)}</dd>
          </div>
          <div>
            <dt>累计</dt>
            <dd aria-label="累计流量">{formatBytes(traffic.totalBytes)}</dd>
          </div>
          <div>
            <dt>节点</dt>
            <dd>{node ? node.name : "尚未导入"}</dd>
          </div>
          <div>
            <dt>协议</dt>
            <dd>{node ? node.protocol : "—"}</dd>
          </div>
          <div>
            <dt>地址</dt>
            <dd>{node ? `${node.server}:${node.port}` : "—"}</dd>
          </div>
          <div>
            <dt>Core</dt>
            <dd>{status ? status.core : "—"}</dd>
          </div>
          <div>
            <dt>模式</dt>
            <dd>
              <select
                aria-label="路由模式"
                disabled={busy || connected || status === null}
                value={status?.mode ?? "global"}
                onChange={(event) =>
                  void run(() =>
                    setRoutingMode(event.target.value as RoutingMode),
                  )
                }
              >
                <option value="global">全局</option>
                <option value="rule">规则</option>
                <option value="direct">直连</option>
              </select>
            </dd>
          </div>
          <div>
            <dt>本地代理</dt>
            <dd>
              {status
                ? `SOCKS ${status.socksPort} · HTTP ${status.httpPort}`
                : "—"}
            </dd>
          </div>
          <div>
            <dt>系统代理</dt>
            <dd>
              {connected && status?.systemProxy
                ? "已接管系统代理"
                : "未接管系统代理"}
            </dd>
          </div>
          <div>
            <dt>TUN</dt>
            <dd>{platform ? TUN_LABEL[platform.tunAvailability] : "—"}</dd>
          </div>
          <div>
            <dt>构建目标</dt>
            <dd>{platform ? platform.artifactIdentifier : platformError}</dd>
          </div>
        </dl>

        <div className="actions">
          <button
            type="button"
            disabled={
              busy ||
              nodeTestInProgress ||
              node === null ||
              systemProxyStartup !== "clean"
            }
            onClick={() =>
              void run(connected ? disconnectSession : connectSession)
            }
          >
            {connected ? "断开" : "连接"}
          </button>
        </div>

              </div>
              <div className="tab-panel" hidden={panel !== "nodes"}>
        <h2>导入节点</h2>

        <textarea
          aria-label="分享链接"
          rows={3}
          value={uri}
          disabled={busy || connected}
          placeholder="vless:// vmess:// trojan:// ss:// hysteria2://"
          onChange={(event) => setUri(event.target.value)}
        />

        <div className="actions">
          <button
            type="button"
            disabled={busy || connected}
            onClick={() => void onImport()}
          >
            导入
          </button>
        </div>

        <h2>批量导入</h2>

        <p className="hint">
          可粘贴多行分享链接，或整体 Base64 的订阅正文。批量导入不会改变当前选中的节点。
        </p>

        <textarea
          aria-label="批量节点列表"
          rows={4}
          value={bulkText}
          disabled={busy || connected}
          placeholder="每行一个链接，或粘贴 Base64 订阅正文"
          onChange={(event) => setBulkText(event.target.value)}
        />

        <div className="actions">
          <button
            type="button"
            disabled={busy || connected}
            onClick={() => void runBulkImport(bulkText)}
          >
            批量导入
          </button>
          <label className="file-import">
            从文件导入
            <input
              aria-label="从文件导入节点"
              type="file"
              accept=".txt,.text,text/plain"
              disabled={busy || connected}
              onChange={(event) => {
                const file = event.target.files?.[0];
                // Clear the value so picking the same file twice still fires.
                event.target.value = "";
                void onImportFile(file);
              }}
            />
          </label>
          <label className="file-import">
            扫描二维码图片
            <input
              aria-label="从二维码图片导入节点"
              type="file"
              accept="image/png,image/jpeg"
              disabled={busy || connected}
              onChange={(event) => {
                const file = event.target.files?.[0];
                event.target.value = "";
                void onImportQrCode(file);
              }}
            />
          </label>
        </div>

        {bulkReport !== null && (
          <div className="bulk-report" role="status" aria-label="批量导入结果">
            <p>
              成功导入 {bulkReport.imported} 个
              {bulkReport.duplicates > 0 &&
                `，跳过 ${bulkReport.duplicates} 个重复`}
              {bulkReport.failures.length > 0 &&
                `，${bulkReport.failures.length} 行失败`}
            </p>
            {bulkReport.failures.length > 0 && (
              <ul>
                {bulkReport.failures.map((failure, index) => (
                  <li key={`${failure.line ?? "none"}-${index}`}>
                    {failure.line === null
                      ? failure.message
                      : `第 ${failure.line} 行：${failure.message}`}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        <h2>手动创建节点</h2>

        <div className="settings-form" aria-label="手动创建节点">
          <label>
            协议
            <select
              aria-label="节点协议"
              value={createForm.protocol}
              disabled={busy || connected}
              onChange={(event) =>
                updateCreateForm({
                  protocol: event.target.value as ProxyProtocol,
                })
              }
            >
              <option value="vless">VLESS</option>
              <option value="vmess">VMess</option>
              <option value="trojan">Trojan</option>
              <option value="shadowsocks">Shadowsocks</option>
              <option value="hysteria2">Hysteria2</option>
            </select>
          </label>

          <label>
            名称
            <input
              aria-label="新建节点名称"
              value={createForm.name}
              disabled={busy || connected}
              onChange={(event) => updateCreateForm({ name: event.target.value })}
            />
          </label>

          <label>
            服务器
            <input
              aria-label="新建节点服务器"
              value={createForm.server}
              disabled={busy || connected}
              onChange={(event) =>
                updateCreateForm({ server: event.target.value })
              }
            />
          </label>

          <label>
            端口
            <input
              aria-label="新建节点端口"
              inputMode="numeric"
              value={createForm.port}
              disabled={busy || connected}
              onChange={(event) => updateCreateForm({ port: event.target.value })}
            />
          </label>

          {(createForm.protocol === "vless" ||
            createForm.protocol === "vmess") && (
            <label>
              UUID
              <input
                aria-label="节点 UUID"
                value={createForm.userId}
                disabled={busy || connected}
                onChange={(event) =>
                  updateCreateForm({ userId: event.target.value })
                }
              />
            </label>
          )}

          {createForm.protocol === "vless" && (
            <label>
              flow
              <input
                aria-label="VLESS flow"
                placeholder="留空表示不使用"
                value={createForm.flow}
                disabled={busy || connected}
                onChange={(event) =>
                  updateCreateForm({ flow: event.target.value })
                }
              />
            </label>
          )}

          {createForm.protocol === "vmess" && (
            <>
              <label>
                加密方式
                <select
                  aria-label="VMess 加密方式"
                  value={createForm.security}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({
                      security: event.target.value as VmessSecurity,
                    })
                  }
                >
                  <option value="Auto">auto</option>
                  <option value="Aes128Gcm">aes-128-gcm</option>
                  <option value="Chacha20Poly1305">chacha20-poly1305</option>
                  <option value="None">none</option>
                  <option value="Zero">zero</option>
                </select>
              </label>
              <label>
                alterId
                <input
                  aria-label="VMess alterId"
                  inputMode="numeric"
                  value={createForm.alterId}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ alterId: event.target.value })
                  }
                />
              </label>
            </>
          )}

          {createForm.protocol === "shadowsocks" && (
            <label>
              加密方式
              <select
                aria-label="Shadowsocks 加密方式"
                value={createForm.method}
                disabled={busy || connected}
                onChange={(event) =>
                  updateCreateForm({ method: event.target.value })
                }
              >
                {SHADOWSOCKS_METHODS.map((method) => (
                  <option key={method} value={method}>
                    {method}
                  </option>
                ))}
              </select>
            </label>
          )}

          {(createForm.protocol === "trojan" ||
            createForm.protocol === "shadowsocks") && (
            <label>
              密码
              <input
                aria-label="节点密码"
                type="password"
                value={createForm.password}
                disabled={busy || connected}
                onChange={(event) =>
                  updateCreateForm({ password: event.target.value })
                }
              />
            </label>
          )}

          {createForm.protocol === "hysteria2" && (
            <>
              <label>
                认证密码
                <input
                  aria-label="Hysteria2 认证密码"
                  type="password"
                  placeholder="留空表示不使用"
                  value={createForm.authentication}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ authentication: event.target.value })
                  }
                />
              </label>
              <label>
                <input
                  aria-label="启用混淆"
                  type="checkbox"
                  checked={createForm.obfsEnabled}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ obfsEnabled: event.target.checked })
                  }
                />
                启用混淆
              </label>
              {createForm.obfsEnabled && (
                <>
                  <label>
                    混淆方式
                    <select
                      aria-label="混淆方式"
                      value={createForm.obfsMethod}
                      disabled={busy || connected}
                      onChange={(event) =>
                        updateCreateForm({
                          obfsMethod: event.target.value as ObfuscationMethod,
                        })
                      }
                    >
                      <option value="Salamander">salamander</option>
                      <option value="Gecko">gecko</option>
                    </select>
                  </label>
                  <label>
                    混淆密码
                    <input
                      aria-label="混淆密码"
                      type="password"
                      value={createForm.obfsPassword}
                      disabled={busy || connected}
                      onChange={(event) =>
                        updateCreateForm({ obfsPassword: event.target.value })
                      }
                    />
                  </label>
                </>
              )}
            </>
          )}

          {usesStreamTransport(createForm.protocol) && (
            <label>
              传输方式
              <select
                aria-label="传输方式"
                value={createForm.transport}
                disabled={
                  busy || connected || createForm.protocol === "shadowsocks"
                }
                onChange={(event) =>
                  updateCreateForm({
                    transport: event.target.value as TransportKind,
                  })
                }
              >
                <option value="tcp">TCP</option>
                <option value="websocket">WebSocket</option>
                <option value="grpc">gRPC</option>
              </select>
            </label>
          )}

          {usesStreamTransport(createForm.protocol) &&
            createForm.transport === "websocket" && (
              <>
                <label>
                  路径
                  <input
                    aria-label="WebSocket 路径"
                    value={createForm.wsPath}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ wsPath: event.target.value })
                    }
                  />
                </label>
                <label>
                  Host
                  <input
                    aria-label="WebSocket Host"
                    placeholder="留空表示不使用"
                    value={createForm.wsHost}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ wsHost: event.target.value })
                    }
                  />
                </label>
              </>
            )}

          {usesStreamTransport(createForm.protocol) &&
            createForm.transport === "grpc" && (
              <>
                <label>
                  serviceName
                  <input
                    aria-label="gRPC serviceName"
                    value={createForm.grpcServiceName}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ grpcServiceName: event.target.value })
                    }
                  />
                </label>
                <label>
                  模式
                  <select
                    aria-label="gRPC 模式"
                    value={createForm.grpcMode}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({
                        grpcMode: event.target.value as GrpcMode,
                      })
                    }
                  >
                    <option value="gun">gun</option>
                    <option value="multi">multi</option>
                    <option value="guna">guna</option>
                  </select>
                </label>
                <label>
                  authority
                  <input
                    aria-label="gRPC authority"
                    placeholder="留空表示不使用"
                    value={createForm.grpcAuthority}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ grpcAuthority: event.target.value })
                    }
                  />
                </label>
              </>
            )}

          {usesStreamTransport(createForm.protocol) &&
            createForm.protocol !== "shadowsocks" && (
              <label>
                <input
                  aria-label="启用 TLS"
                  type="checkbox"
                  checked={createForm.tlsEnabled}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ tlsEnabled: event.target.checked })
                  }
                />
                启用 TLS
              </label>
            )}

          {(createForm.tlsEnabled || createForm.protocol === "hysteria2") &&
            createForm.protocol !== "shadowsocks" && (
              <>
                <label>
                  SNI
                  <input
                    aria-label="TLS SNI"
                    placeholder="留空表示使用服务器地址"
                    value={createForm.serverName}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ serverName: event.target.value })
                    }
                  />
                </label>
                <label>
                  ALPN
                  <input
                    aria-label="TLS ALPN"
                    placeholder="逗号分隔，如 h2,http/1.1"
                    value={createForm.alpn}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ alpn: event.target.value })
                    }
                  />
                </label>
                <label>
                  指纹
                  <input
                    aria-label="TLS 指纹"
                    placeholder="留空表示不使用"
                    value={createForm.fingerprint}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ fingerprint: event.target.value })
                    }
                  />
                </label>
                <label>
                  <input
                    aria-label="允许不安全证书"
                    type="checkbox"
                    checked={createForm.allowInsecure}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ allowInsecure: event.target.checked })
                    }
                  />
                  允许不安全证书
                </label>
              </>
            )}

          <label>
            <input
              aria-label="启用 UDP"
              type="checkbox"
              checked={createForm.udpEnabled}
              disabled={busy || connected}
              onChange={(event) =>
                updateCreateForm({ udpEnabled: event.target.checked })
              }
            />
            启用 UDP
          </label>

          <div className="actions">
            <button
              type="button"
              disabled={busy || connected}
              onClick={() => void onCreateNode()}
            >
              创建节点
            </button>
            <button
              type="button"
              disabled={busy || connected}
              onClick={() => setCreateForm(emptyManualNodeForm)}
            >
              重置
            </button>
          </div>
        </div>

              </div>
              <div className="tab-panel" hidden={panel !== "subscriptions"}>
        <h2>订阅</h2>

        <div className="actions">
          <button
            type="button"
            disabled={
              busy ||
              connected ||
              nodeTestInProgress ||
              subscriptions.length === 0
            }
            onClick={() => void onRefreshAllSubscriptions()}
          >
            全部更新
          </button>
        </div>

        {subscriptions.length === 0 ? (
          <p className="hint">尚未添加订阅</p>
        ) : (
          <table className="node-list" aria-label="订阅列表">
            <thead>
              <tr>
                <th>名称</th>
                <th>节点</th>
                <th>更新</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {subscriptions.map((item) => (
                <tr key={item.id}>
                  <td>{item.name}</td>
                  <td>{item.nodeCount}</td>
                  <td>
                    {item.lastError ??
                      (item.lastUpdatedAt === null ? "从未" : "已更新")}
                  </td>
                  <td className="node-actions">
                    <button
                      type="button"
                      aria-label={`编辑 ${item.name}`}
                      disabled={busy || connected || nodeTestInProgress}
                      onClick={() => onEditSubscription(item)}
                    >
                      编辑
                    </button>
                    <button
                      type="button"
                      aria-label={`刷新 ${item.name}`}
                      disabled={
                        busy || connected || nodeTestInProgress || !item.enabled
                      }
                      onClick={() => void onRefreshSubscription(item.id)}
                    >
                      刷新
                    </button>
                    <button
                      type="button"
                      aria-label={`删除订阅 ${item.name}`}
                      disabled={busy || connected || nodeTestInProgress}
                      onClick={() => void onDeleteSubscription(item.id)}
                    >
                      删除
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        <div className="subscription-form">
          <label>
            名称
            <input
              aria-label="订阅名称"
              value={subscriptionName}
              disabled={busy}
              onChange={(event) => setSubscriptionName(event.target.value)}
            />
          </label>
          <label>
            地址
            <input
              aria-label="订阅地址"
              type="password"
              value={subscriptionUrl}
              disabled={busy}
              placeholder={
                editingSubscriptionId === null ? "https://" : "留空则不修改"
              }
              onChange={(event) => setSubscriptionUrl(event.target.value)}
            />
          </label>
          <label>
            更新间隔（分钟）
            <input
              aria-label="更新间隔"
              type="number"
              min="1"
              value={subscriptionInterval}
              disabled={busy}
              onChange={(event) => setSubscriptionInterval(event.target.value)}
            />
          </label>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={subscriptionAutoUpdate}
              disabled={busy}
              onChange={(event) =>
                setSubscriptionAutoUpdate(event.target.checked)
              }
            />
            自动更新
          </label>
          {editingSubscriptionId !== null && (
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={subscriptionEnabled}
                disabled={busy}
                onChange={(event) => setSubscriptionEnabled(event.target.checked)}
              />
              启用订阅
            </label>
          )}
        </div>

        <div className="actions">
          <button
            type="button"
            disabled={busy}
            onClick={() => void onSaveSubscription()}
          >
            {editingSubscriptionId === null ? "添加订阅" : "保存修改"}
          </button>
          {editingSubscriptionId !== null && (
            <button
              type="button"
              disabled={busy}
              onClick={resetSubscriptionForm}
            >
              取消
            </button>
          )}
        </div>

              </div>
              <div className="tab-panel" hidden={panel !== "routing"}>
        <h2>路由规则</h2>

        <p className="hint">
          运行顺序固定为：本地安全规则 → 用户规则 → Geo 规则 → 默认出口。仅规则模式应用列表。
        </p>
        {routeDraft === null ? (
          <p className="hint">正在读取路由设置</p>
        ) : (
          <>
            <div className="settings-form">
              <label>
                规则类型
                <select
                  aria-label="规则类型"
                  disabled={busy || connected}
                  value={routeRuleKind}
                  onChange={(event) =>
                    setRouteRuleKind(event.target.value as RouteRuleKind)
                  }
                >
                  {Object.entries(ROUTE_KIND_LABEL).map(([value, label]) => (
                    <option key={value} value={value}>
                      {label}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                规则值
                <input
                  aria-label="规则值"
                  disabled={busy || connected}
                  placeholder={routeRuleKind === "network" ? "tcp 或 udp" : undefined}
                  value={routeRuleValue}
                  onChange={(event) => setRouteRuleValue(event.target.value)}
                />
              </label>
              <label>
                出口
                <select
                  aria-label="规则出口"
                  disabled={busy || connected}
                  value={routeRuleOutbound}
                  onChange={(event) =>
                    setRouteRuleOutbound(event.target.value as RouteOutbound)
                  }
                >
                  <option value="proxy">代理</option>
                  <option value="direct">直连</option>
                </select>
              </label>
            </div>
            <div className="actions">
              <button
                type="button"
                disabled={busy || connected}
                onClick={onAddRouteRule}
              >
                添加规则
              </button>
            </div>

            {routeDraft.rules.length === 0 ? (
              <p className="hint">尚未添加规则</p>
            ) : (
              <table className="node-list" aria-label="路由规则列表">
                <thead>
                  <tr>
                    <th>顺序</th>
                    <th>类型</th>
                    <th>值</th>
                    <th>出口</th>
                    <th>启用</th>
                    <th>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {routeDraft.rules.map((rule, index) => (
                    <tr key={`${rule.kind}-${rule.value}-${index}`}>
                      <td>{index + 1}</td>
                      <td>{ROUTE_KIND_LABEL[rule.kind]}</td>
                      <td>{rule.value}</td>
                      <td>{rule.outbound === "proxy" ? "代理" : "直连"}</td>
                      <td>
                        <input
                          aria-label={`启用规则 ${index + 1}`}
                          checked={rule.enabled}
                          disabled={busy || connected}
                          type="checkbox"
                          onChange={(event) => {
                            const rules = [...routeDraft.rules];
                            rules[index] = {
                              ...rule,
                              enabled: event.target.checked,
                            };
                            setRouteDraft({ ...routeDraft, rules });
                            setRouteDirty(true);
                          }}
                        />
                      </td>
                      <td className="node-actions">
                        <button
                          type="button"
                          disabled={
                            busy ||
                            connected ||
                            index === 0 ||
                            isGeoRule(rule.kind) !==
                              isGeoRule(routeDraft.rules[index - 1].kind)
                          }
                          onClick={() => onMoveRouteRule(index, -1)}
                        >
                          上移
                        </button>
                        <button
                          type="button"
                          disabled={
                            busy ||
                            connected ||
                            index === routeDraft.rules.length - 1 ||
                            isGeoRule(rule.kind) !==
                              isGeoRule(routeDraft.rules[index + 1].kind)
                          }
                          onClick={() => onMoveRouteRule(index, 1)}
                        >
                          下移
                        </button>
                        <button
                          type="button"
                          disabled={busy || connected}
                          onClick={() => {
                            setRouteDraft({
                              ...routeDraft,
                              rules: routeDraft.rules.filter(
                                (_, ruleIndex) => ruleIndex !== index,
                              ),
                            });
                            setRouteDirty(true);
                          }}
                        >
                          删除
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}

            <div className="settings-form">
              <label>
                默认出口
                <select
                  aria-label="默认出口"
                  disabled={busy || connected}
                  value={routeDraft.finalOutbound}
                  onChange={(event) => {
                    setRouteDraft({
                      ...routeDraft,
                      finalOutbound: event.target.value as RouteOutbound,
                    });
                    setRouteDirty(true);
                  }}
                >
                  <option value="proxy">代理</option>
                  <option value="direct">直连</option>
                </select>
              </label>
            </div>
          </>
        )}

        <div className="actions">
          <button
            type="button"
            disabled={busy || connected || routeDraft === null || !routeDirty}
            onClick={() => void onSaveRoute()}
          >
            保存路由
          </button>
        </div>

        <h2>DNS</h2>

        {dnsDraft === null ? (
          <p className="hint">正在读取 DNS 设置</p>
        ) : (
          <div className="settings-form">
            <label>
              模式
              <select
                aria-label="DNS 模式"
                disabled={busy || connected}
                value={dnsDraft.mode}
                onChange={(event) => {
                  const mode = event.target.value as DnsMode;
                  const upstream =
                    mode === "doh"
                      ? { port: 443, server: "cloudflare-dns.com" }
                      : mode === "dot"
                        ? {
                            port: 853,
                            server: "1dot1dot1dot1.cloudflare-dns.com",
                          }
                        : mode === "plainUdp" || mode === "plainTcp"
                          ? { port: 53, server: "1.1.1.1" }
                          : {};
                  setDnsDraft((current) =>
                    current === null ? null : { ...current, ...upstream, mode },
                  );
                  setDnsDirty(true);
                }}
              >
                <option value="system">系统 DNS</option>
                <option value="plainUdp">UDP</option>
                <option value="plainTcp">TCP</option>
                <option value="doh">DoH</option>
                <option value="dot">DoT</option>
              </select>
            </label>
            {dnsDraft.mode !== "system" && (
              <>
                <label>
                  服务器
                  <input
                    aria-label="DNS 服务器"
                    disabled={busy || connected}
                    value={dnsDraft.server}
                    onChange={(event) => {
                      setDnsDraft({ ...dnsDraft, server: event.target.value });
                      setDnsDirty(true);
                    }}
                  />
                </label>
                <label>
                  端口
                  <input
                    aria-label="DNS 端口"
                    disabled={busy || connected}
                    min="1"
                    max="65535"
                    type="number"
                    value={dnsDraft.port}
                    onChange={(event) => {
                      setDnsDraft({ ...dnsDraft, port: Number(event.target.value) });
                      setDnsDirty(true);
                    }}
                  />
                </label>
              </>
            )}
            {dnsDraft.mode === "doh" && (
              <label>
                DoH 路径
                <input
                  aria-label="DoH 路径"
                  disabled={busy || connected}
                  value={dnsDraft.dohPath}
                  onChange={(event) => {
                    setDnsDraft({ ...dnsDraft, dohPath: event.target.value });
                    setDnsDirty(true);
                  }}
                />
              </label>
            )}
            <label>
              地址策略
              <select
                aria-label="DNS 地址策略"
                disabled={busy || connected}
                value={dnsDraft.strategy}
                onChange={(event) => {
                  setDnsDraft({
                    ...dnsDraft,
                    strategy: event.target.value as DnsStrategy,
                  });
                  setDnsDirty(true);
                }}
              >
                <option value="preferIpv4">优先 IPv4</option>
                <option value="preferIpv6">优先 IPv6</option>
                <option value="ipv4Only">仅 IPv4</option>
                <option value="ipv6Only">仅 IPv6</option>
              </select>
            </label>
            <label>
              系统 DNS 域名后缀
              <textarea
                aria-label="系统 DNS 域名后缀"
                disabled={busy || connected}
                rows={2}
                placeholder="每行一个，例如 lan"
                value={dnsDraft.systemDomains.join("\n")}
                onChange={(event) => {
                  setDnsDraft({
                    ...dnsDraft,
                    systemDomains: event.target.value.split("\n"),
                  });
                  setDnsDirty(true);
                }}
              />
            </label>
            <label className="checkbox-label">
              <input
                aria-label="启用 IPv6 DNS"
                checked={dnsDraft.ipv6Enabled}
                disabled={busy || connected}
                type="checkbox"
                onChange={(event) => {
                  setDnsDraft({ ...dnsDraft, ipv6Enabled: event.target.checked });
                  setDnsDirty(true);
                }}
              />
              IPv6
            </label>
            <label className="checkbox-label">
              <input
                aria-label="启用 FakeIP"
                checked={dnsDraft.fakeIpEnabled}
                disabled={busy || connected}
                type="checkbox"
                onChange={(event) => {
                  setDnsDraft({ ...dnsDraft, fakeIpEnabled: event.target.checked });
                  setDnsDirty(true);
                }}
              />
              FakeIP
            </label>
          </div>
        )}

        <div className="actions">
          <button
            type="button"
            disabled={busy || connected || dnsDraft === null || !dnsDirty}
            onClick={() => void onSaveDns()}
          >
            保存 DNS
          </button>
        </div>

              </div>
              <div className="tab-panel" hidden={panel !== "settings"}>
        <h2>设置</h2>

        {settings === null ? (
          <p className="hint">正在读取设置…</p>
        ) : (
          <div className="settings-form" aria-label="应用设置">
            <label className="checkbox-label">
              <input
                aria-label="启用 TUN"
                type="checkbox"
                checked={settings.tunEnabled}
                disabled={
                  busy ||
                  connected ||
                  platform?.tunAvailability === "unavailableInUnsignedBuild"
                }
                onChange={(event) =>
                  void onChangeSettings({ tunEnabled: event.target.checked })
                }
              />
              使用 TUN 接管全局流量
            </label>
            <p className="hint">
              {platform ? TUN_NOTICE[platform.tunAvailability] : ""}
              TUN 与系统代理互斥，启用后本次会话不会修改系统代理。
            </p>
            <label className="checkbox-label">
              <input
                aria-label="开机启动"
                type="checkbox"
                checked={settings.launchAtLogin}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({ launchAtLogin: event.target.checked })
                }
              />
              登录系统时自动启动 MgClash
            </label>
            <label className="checkbox-label">
              <input
                aria-label="启动时自动连接"
                type="checkbox"
                checked={settings.connectOnLaunch}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({
                    connectOnLaunch: event.target.checked,
                  })
                }
              />
              启动时自动连接上次选中的节点
            </label>
            <label className="checkbox-label">
              <input
                aria-label="关闭时最小化到托盘"
                type="checkbox"
                checked={settings.closeToTray}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({ closeToTray: event.target.checked })
                }
              />
              关闭窗口时最小化到托盘，而不是退出
            </label>
            <label>
              Core
              <select
                aria-label="Core 选择"
                value={settings.corePreference}
                disabled={busy || connected}
                onChange={(event) =>
                  void onChangeSettings({
                    corePreference: event.target.value as CorePreference,
                  })
                }
              >
                <option value="auto">自动</option>
                <option value="sing-box">sing-box</option>
                <option value="xray">Xray</option>
              </select>
            </label>
            <p className="hint">
              自动模式按节点协议和能力矩阵决定。Xray 不支持 Hysteria2，选中后遇到该协议的节点会提示原因。
            </p>
            <label>
              默认日志级别
              <select
                aria-label="默认日志级别"
                value={settings.logLevel}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({
                    logLevel: event.target.value as LogLevel,
                  })
                }
              >
                <option value="error">error</option>
                <option value="warn">warn</option>
                <option value="info">info</option>
                <option value="debug">debug</option>
                <option value="trace">trace</option>
              </select>
            </label>
          </div>
        )}

              </div>
              <div className="tab-panel" hidden={panel !== "logs"}>
        <h2>诊断</h2>

        <p className="hint">
          导出的诊断包已按 PRD 25.3 脱敏：凭据字段一律替换为 [REDACTED]。
        </p>

        <div className="actions">
          <button type="button" disabled={busy} onClick={() => void onExport()}>
            导出诊断
          </button>
        </div>

        {exportedTo !== null && (
          <p className="success" role="status">
            已导出到 {exportedTo}
          </p>
        )}

        {error !== null && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
              </div>
            </div>
          </div>
        </div>
      </section>

      <footer className="statusbar">
        <span>
          本地 SOCKS {status?.socksPort ?? "—"} · HTTP {status?.httpPort ?? "—"}
        </span>
        <span>Core {status?.core ?? "—"}</span>
        <label className="status-control">
          路由
          <select
            aria-label="状态栏路由模式"
            disabled={busy || connected || status === null}
            value={status?.mode ?? "global"}
            onChange={(event) =>
              void run(() => setRoutingMode(event.target.value as RoutingMode))
            }
          >
            <option value="global">全局</option>
            <option value="rule">规则</option>
            <option value="direct">直连</option>
          </select>
        </label>
        <label className="status-control">
          <input
            aria-label="状态栏 TUN"
            type="checkbox"
            checked={settings?.tunEnabled ?? false}
            disabled={
              busy ||
              connected ||
              settings === null ||
              platform?.tunAvailability === "unavailableInUnsignedBuild"
            }
            onChange={(event) =>
              void onChangeSettings({ tunEnabled: event.target.checked })
            }
          />
          TUN
        </label>
        <label className="status-control">
          系统代理
          <select
            aria-label="状态栏系统代理"
            disabled={busy || connected || settings === null}
            value={settings?.systemProxyMode ?? "managed"}
            onChange={(event) =>
              void onChangeSettings({
                systemProxyMode: event.target.value as SystemProxyMode,
              })
            }
          >
            <option value="managed">自动配置</option>
            <option value="pac">PAC</option>
            <option value="cleared">清除</option>
            <option value="unchanged">不改变</option>
          </select>
        </label>
        <span className="statusbar-rates">
          ↓ {formatRate(traffic.downloadBytesPerSecond)} ↑{" "}
          {formatRate(traffic.uploadBytesPerSecond)}
        </span>
        <span className={connected ? "badge on" : "badge off"}>
          {connected ? "已连接" : "未连接"}
        </span>
      </footer>
    </main>
  );
}
