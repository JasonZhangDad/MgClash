import { DEFAULT_LOCALE, translate, type Locale } from "./i18n";
import { useCallback, useEffect, useRef, useState } from "react";

import {
  buildManualNodeDraft,
  blankManualNodeForm,
  emptyManualNodeForm,
  formFromManualNodeDraft,
  SHADOWSOCKS_METHODS,
  usesStreamTransport,
  type GrpcMode,
  type KcpHeaderType,
  type ManualNodeForm,
  type TransportKind,
  type XhttpMode,
} from "./manualNode";
import { loadPlatformSummary, type PlatformSummary } from "./platform";
import {
  connectSession,
  createNode,
  cloneNode,
  deleteNode,
  checkUpdate,
  checkCoreUpdate,
  downloadCoreUpdate,
  loadGeoAssetsStatus,
  updateGeoAssets,
  nodeQrCode,
  readQrCode,
  exportNodeLink,
  removeDuplicateNodes,
  dismissSystemProxyRecovery,
  disconnectSession,
  exportDiagnostics,
  exportPreferences,
  exportProfile,
  importPreferences,
  importProfile,
  clearLogs,
  clearTraffic,
  importNode,
  importNodes,
  loadAppSettings,
  loadLogs,
  loadNodeDraft,
  saveAppSettings,
  loadNodeGroups,
  loadNodes,
  loadSessionStatus,
  loadSystemProxyStartupStatus,
  loadNodeTraffic,
  loadTraffic,
  moveNode,
  reorderNodes,
  recoverSystemProxy,
  selectNode,
  switchNode,
  setDnsSettings,
  setNodeEnabled,
  setNodeGroup,
  setNodeGroupStrategy as saveNodeGroupStrategy,
  setRouteSettings,
  setRouteScheme,
  createRouteScheme,
  deleteRouteScheme,
  setRoutingMode,
  testAllNodes,
  testDownloadSpeed,
  testNode,
  testUrl,
  updateNode,
  type AppSettings,
  type BulkImportReport,
  type CorePreference,
  type LogEntry,
  type LogLevel,
  type LogSource,
  type NodeSummary,
  type NodeGroupStrategy,
  type NodeGroupSummary,
  type NodeTestResult,
  type ObfuscationMethod,
  type ProxyProtocol,
  type SpeedTestResult,
  type VmessSecurity,
  type DnsMode,
  type DnsSettings,
  type DnsStrategy,
  type DnsTemplate,
  type RouteOutbound,
  type RouteRuleKind,
  type RouteSettings,
  type SessionStatus,
  type SystemProxyStartupStatus,
  type NodeTraffic,
  type UpdateCheck,
  type CoreUpdateCheck,
  type CoreVersionCheck,
  type InstalledCoreEntry,
  type GeoAssetsStatus,
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
import {
  describeFailure,
  DEFAULT_SPEED_TEST_URL,
  DEFAULT_URL_TEST_ADDRESS,
  formatBytes,
  formatClock,
  formatRate,
  isGeoRule,
  LAYOUT_KEY,
  latencyQuality,
  noticeWasDismissed,
  regionFlag,
  ROUTE_KIND_LABEL,
  runtimeOrderedRoute,
  savedLayout,
  savedTheme,
  savedUrlTestAddress,
  sortNodeIdsByLatency,
  lowestLatencyNodeId,
  matchesHotkey,
  THEME_KEY,
  TRAFFIC_REFRESH_INTERVAL_MS,
  TUN_NOTICE,
  REFRESH_INTERVAL_MS,
  TUN_LABEL,
  UNSIGNED_NOTICE_KEY,
  URL_TEST_ADDRESS_KEY,
  type DialogId,
  type MainLayout,
  type NodeMenuPosition,
  type ThemeMode,
} from "./appHelpers";
import {
  clearGlobalHotkeys,
  syncGlobalHotkeys,
} from "./globalHotkeys";
import { MenuBar } from "./components/MenuBar";
import { StatusBar } from "./components/StatusBar";
import { MsgView } from "./components/MsgView";
import { Dialog } from "./components/Dialog";

function nodeGroupStrategyBadge(
  strategy: NodeGroupStrategy,
  t: (text: string) => string,
): string {
  switch (strategy) {
    case "urlTest":
      return t("自动");
    case "fallback":
      return t("故障转移");
    case "loadBalance":
      return t("负载均衡");
    default:
      return "";
  }
}

const EMPTY_TRAFFIC: TrafficSnapshot = {
  downloadBytesPerSecond: 0,
  monthBytes: 0,
  todayBytes: 0,
  totalBytes: 0,
  uploadBytesPerSecond: 0,
};

export default function App() {
  const [locale, setLocale] = useState<Locale>(DEFAULT_LOCALE);
  /// Every visible string goes through here; the source text is the key.
  const t = useCallback(
    (text: string) => translate(locale, text),
    [locale],
  );

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
  const [update, setUpdate] = useState<UpdateCheck | null>(null);
  const [coreUpdate, setCoreUpdate] = useState<CoreUpdateCheck | null>(null);
  const [geoStatus, setGeoStatus] = useState<GeoAssetsStatus | null>(null);
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
  const [nodeGroupStrategy, setNodeGroupStrategy] =
    useState<NodeGroupStrategy>("select");
  const [editingNodeId, setEditingNodeId] = useState<string | null>(null);
  const [subscriptions, setSubscriptions] = useState<SubscriptionSummary[]>([]);
  const [uri, setUri] = useState("");
  const [createForm, setCreateForm] =
    useState<ManualNodeForm>(emptyManualNodeForm);
  const [dialog, setDialog] = useState<DialogId>(null);
  const [theme, setTheme] = useState<ThemeMode>(() => savedTheme());
  const [layout, setLayout] = useState<MainLayout>(() => savedLayout());
  const [msgVisible, setMsgVisible] = useState(true);
  const [mainTab, setMainTab] = useState<"profiles" | "msg">("profiles");
  const [nodeQuery, setNodeQuery] = useState("");
  const [inspectedId, setInspectedId] = useState<string | null>(null);
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
  const [subscriptionUserAgent, setSubscriptionUserAgent] = useState("");
  const [subscriptionSubconverter, setSubscriptionSubconverter] = useState("");
  const [subscriptionInclude, setSubscriptionInclude] = useState("");
  const [subscriptionExclude, setSubscriptionExclude] = useState("");
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
  const [nodeSpeeds, setNodeSpeeds] = useState<
    Record<string, SpeedTestResult | { status: "testing" }>
  >({});
  const [testingAllNodes, setTestingAllNodes] = useState(false);
  const [testingSpeed, setTestingSpeed] = useState(false);
  const cancelNodeTests = useRef(false);
  const [noticeDismissed, setNoticeDismissed] = useState(noticeWasDismissed);

  const nodeTestInProgress =
    testingSpeed ||
    Object.values(nodeTests).some((result) => result.status === "testing");

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      // The theme still applies to this launch even if it cannot be stored.
    }
  }, [theme]);

  useEffect(() => {
    try {
      localStorage.setItem(LAYOUT_KEY, layout);
    } catch {
      // The layout still applies to this launch even if it cannot be stored.
    }
  }, [layout]);

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

  const run = useCallback(
    async (command: () => Promise<SessionStatus | void>) => {
      setBusy(true);
      setError(null);
      setExportedTo(null);
      try {
        const next = await command();
        if (next !== undefined) {
          setStatus(next);
        }
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

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
        const legacy = savedUrlTestAddress();
        const migrated =
          loaded.urlTestAddress === DEFAULT_URL_TEST_ADDRESS &&
          legacy !== DEFAULT_URL_TEST_ADDRESS
            ? { ...loaded, urlTestAddress: legacy }
            : loaded;
        setSettings(migrated);
        setLogLevel(migrated.logLevel);
        setLocale(migrated.locale);
        setUrlTestAddress(migrated.urlTestAddress);
        if (migrated !== loaded) {
          void saveAppSettings(migrated).then(
            (saved) => setSettings(saved),
            (failure: unknown) => setError(describeFailure(failure)),
          );
        }
        try {
          localStorage.removeItem(URL_TEST_ADDRESS_KEY);
        } catch {
          // Ignoring storage failures keeps settings usable.
        }
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
        if (changes.locale !== undefined) {
          setLocale(changes.locale);
        }
        if (changes.urlTestAddress !== undefined) {
          setUrlTestAddress(changes.urlTestAddress);
        }
      } catch (failure: unknown) {
        // Put the stored values back so the switches never claim a state the
        // app did not persist.
        setSettings(settings);
        setLocale(settings.locale);
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
      if (editingNodeId === null) {
        setStatus(await createNode(result.draft));
      } else {
        setStatus(await updateNode(editingNodeId, result.draft));
      }
      setNodes(await loadNodes());
      setCreateForm(
        blankManualNodeForm({
          allowInsecure: settings?.defAllowInsecure,
          fingerprint: settings?.defFingerprint,
        }),
      );
      setEditingNodeId(null);
      setDialog(null);
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [createForm, editingNodeId]);

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

  const onCheckUpdate = useCallback(async () => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    setUpdate(null);
    try {
      setUpdate(await checkUpdate());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const formatCoreVersionLine = (entry: CoreVersionCheck) => {
    const source = entry.fromBinary
      ? entry.current
      : `${entry.current}（${t("未检测到已配置的二进制，显示本构建期望版本")}）`;
    if (entry.updateAvailable) {
      return t("{name} 有新版本 {latest}，当前 {current}")
        .replace("{name}", entry.name)
        .replace("{latest}", entry.latest)
        .replace("{current}", source);
    }
    return t("{name} 已是最新版本 {current}")
      .replace("{name}", entry.name)
      .replace("{current}", source);
  };

  const formatInstalledCoreLine = (name: string, entry: InstalledCoreEntry | undefined) => {
    if (entry === undefined) {
      return t("{name} 未通过应用内安装").replace("{name}", name);
    }
    return t("应用内已安装 {name} {version}")
      .replace("{name}", name)
      .replace("{version}", entry.version);
  };

  const onCheckCoreUpdate = useCallback(async () => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    setCoreUpdate(null);
    try {
      setCoreUpdate(await checkCoreUpdate());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [t]);

  const onDownloadCore = useCallback(
    async (core: "sing-box" | "xray") => {
      setBusy(true);
      setError(null);
      setExportedTo(null);
      try {
        await downloadCoreUpdate(core);
        setCoreUpdate(await checkCoreUpdate());
        setExportedTo(t("Core 已下载并安装，下次连接时生效"));
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [t],
  );

  const onOpenGeo = useCallback(async () => {
    setDialog("geo");
    setError(null);
    try {
      setGeoStatus(await loadGeoAssetsStatus());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    }
  }, []);

  const onUpdateGeo = useCallback(async () => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      const next = await updateGeoAssets();
      setGeoStatus(next);
      setExportedTo(t("Geo 文件已更新"));
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [t]);

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
    setCreateForm(
      blankManualNodeForm({
        allowInsecure: settings?.defAllowInsecure,
        fingerprint: settings?.defFingerprint,
      }),
    );
  }, [settings?.defAllowInsecure, settings?.defFingerprint]);

  const openCreateForm = useCallback(
    (protocol?: ProxyProtocol) => {
      setEditingNodeId(null);
      setCreateForm({
        ...blankManualNodeForm({
          allowInsecure: settings?.defAllowInsecure,
          fingerprint: settings?.defFingerprint,
        }),
        ...(protocol === undefined
          ? {}
          : {
              protocol,
              // AnyTLS / Naive have no plaintext mode; open with TLS already on.
              ...(protocol === "anytls" || protocol === "naive"
                ? { tlsEnabled: true }
                : {}),
              ...(protocol === "custom"
                ? {
                    customCore: "sing-box" as const,
                    customDocument: "",
                    server: "127.0.0.1",
                    port: "443",
                  }
                : {}),
            }),
      });
      setDialog("create");
    },
    [settings?.defAllowInsecure, settings?.defFingerprint],
  );

  const onClearTraffic = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setTraffic(await clearTraffic());
      setNodeTraffic({});
      setExportedTo("流量统计已清除");
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onEditNode = useCallback(async (candidate: NodeSummary) => {
    setBusy(true);
    setError(null);
    try {
      const draft = await loadNodeDraft(candidate.id);
      setCreateForm(formFromManualNodeDraft(draft));
      setEditingNodeId(candidate.id);
      setDialog("create");
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

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

  const onSortNodesByLatency = useCallback(async () => {
    if (nodes.length < 2) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setNodes(await reorderNodes(sortNodeIdsByLatency(nodes)));
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [nodes]);

  const resetNodeGroupForm = useCallback(() => {
    setGroupingNodeId(null);
    setNodeGroupName("");
    setNodeGroupStrategy("select");
  }, []);

  const onGroupNode = useCallback(
    (candidate: NodeSummary) => {
      setGroupingNodeId(candidate.id);
      const group = nodeGroups.find((item) => item.id === candidate.groupId);
      setNodeGroupName(group?.name ?? "");
      setNodeGroupStrategy(group?.strategy ?? "select");
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
      let groups = await loadNodeGroups();
      if (groupName) {
        const group = groups.find((item) => item.name === groupName);
        if (group) {
          groups = await saveNodeGroupStrategy(group.id, nodeGroupStrategy);
        }
      }
      setNodeGroups(groups);
      resetNodeGroupForm();
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [groupingNodeId, nodeGroupName, nodeGroupStrategy, resetNodeGroupForm]);

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
    const enabledIds = nodes
      .filter((candidate) => candidate.enabled)
      .map((candidate) => candidate.id);
    if (enabledIds.length === 0) {
      setError("没有可测速的启用节点");
      return;
    }
    cancelNodeTests.current = false;
    setError(null);
    setTestingAllNodes(true);
    setNodeTests(
      Object.fromEntries(enabledIds.map((id) => [id, { status: "testing" }])),
    );
    const results: NodeTestResult[] = [];
    try {
      await testAllNodes(
        enabledIds,
        (result) => {
          results.push(result);
          setNodeTests((current) => ({ ...current, [result.id]: result }));
        },
        () => cancelNodeTests.current,
      );
      if (settings?.autoSelectLowestLatency) {
        const measured = results
          .filter(
            (result) =>
              result.status === "success" && result.latencyMs !== null,
          )
          .map((result) => ({
            enabled: true,
            id: result.id,
            latencyMs: result.latencyMs,
          }));
        const bestId = lowestLatencyNodeId(measured);
        if (bestId !== null && bestId !== status?.node?.id) {
          setStatus(await switchNode(bestId));
          setNodes(await loadNodes());
        }
      }
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
  }, [nodes, status?.node?.id, settings?.autoSelectLowestLatency]);

  /// Latency-tests a subset, reusing the cancellable batch the toolbar drives.
  const onTestNodes = useCallback(async (ids: string[]) => {
    const enabledIds = ids.filter((id) =>
      nodes.some((candidate) => candidate.id === id && candidate.enabled),
    );
    if (enabledIds.length === 0) {
      setError("没有可测速的启用节点");
      return;
    }
    cancelNodeTests.current = false;
    setError(null);
    setTestingAllNodes(true);
    setNodeTests(
      Object.fromEntries(enabledIds.map((id) => [id, { status: "testing" }])),
    );
    try {
      await testAllNodes(
        enabledIds,
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

  const onSelectLowestLatency = useCallback(async () => {
    const bestId = lowestLatencyNodeId(nodes);
    if (bestId === null) {
      setError("没有可用的延迟结果");
      return;
    }
    if (bestId === status?.node?.id) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setStatus(await switchNode(bestId));
      setNodes(await loadNodes());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [nodes, status?.node?.id]);

  const onStepNode = useCallback(
    async (direction: -1 | 1) => {
      const enabled = nodes.filter((candidate) => candidate.enabled);
      if (enabled.length < 2) {
        return;
      }
      const currentId = status?.node?.id;
      const currentIndex = enabled.findIndex((candidate) => candidate.id === currentId);
      const nextIndex =
        currentIndex < 0
          ? direction > 0
            ? 0
            : enabled.length - 1
          : (currentIndex + direction + enabled.length) % enabled.length;
      const next = enabled[nextIndex];
      if (next === undefined || next.id === currentId) {
        return;
      }
      setBusy(true);
      setError(null);
      try {
        setStatus(await switchNode(next.id));
        setNodes(await loadNodes());
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [nodes, status?.node?.id],
  );

  const onExportPreferences = useCallback(async () => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setExportedTo(await exportPreferences());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onImportPreferences = useCallback(async () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "application/json,.json";
    const selected = await new Promise<File | null>((resolve) => {
      input.addEventListener("change", () => {
        resolve(input.files?.[0] ?? null);
      });
      input.addEventListener("cancel", () => resolve(null));
      input.click();
    });
    if (selected === null) {
      return;
    }
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      // The Tauri command reads a filesystem path; the webview File object
      // exposes one in the desktop shell.
      const path = (selected as File & { path?: string }).path;
      if (path === undefined || path === "") {
        setError(t("无法读取所选文件路径"));
        return;
      }
      const next = await importPreferences(path);
      setSettings(next);
      setLocale(next.locale);
      setLogLevel(next.logLevel);
      setUrlTestAddress(next.urlTestAddress);
      setStatus(await loadSessionStatus());
      setExportedTo(t("设置已从文件恢复"));
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [t]);

  const onExportProfile = useCallback(async () => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setExportedTo(await exportProfile());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onImportProfile = useCallback(async () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "application/json,.json";
    const selected = await new Promise<File | null>((resolve) => {
      input.addEventListener("change", () => {
        resolve(input.files?.[0] ?? null);
      });
      input.addEventListener("cancel", () => resolve(null));
      input.click();
    });
    if (selected === null) {
      return;
    }
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      const path = (selected as File & { path?: string }).path;
      if (path === undefined || path === "") {
        setError(t("无法读取所选文件路径"));
        return;
      }
      const result = await importProfile(path);
      setSettings(result.app);
      setLocale(result.app.locale);
      setLogLevel(result.app.logLevel);
      setUrlTestAddress(result.app.urlTestAddress);
      setStatus(await loadSessionStatus());
      setNodes(await loadNodes());
      setSubscriptions(await loadSubscriptions());
      setExportedTo(
        t("完整配置已恢复（{manual} 个手动节点，{subs} 个订阅）")
          .replace("{manual}", String(result.manualNodeCount))
          .replace("{subs}", String(result.subscriptionCount)),
      );
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [t]);

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
      if (settings !== null && address !== settings.urlTestAddress) {
        await onChangeSettings({ urlTestAddress: address });
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
  }, [status, urlTestAddress, settings, onChangeSettings]);

  const onSpeedTest = useCallback(async () => {
    const address =
      (settings?.speedTestUrl ?? DEFAULT_SPEED_TEST_URL).trim() ||
      DEFAULT_SPEED_TEST_URL;
    const selectedNode = status?.node;
    if (!status?.connected || selectedNode === null || selectedNode === undefined) {
      setError("请先连接节点");
      return;
    }
    setError(null);
    setTestingSpeed(true);
    setNodeSpeeds((current) => ({
      ...current,
      [selectedNode.id]: { status: "testing" },
    }));
    try {
      const result = await testDownloadSpeed(address);
      setNodeSpeeds((current) => ({ ...current, [result.id]: result }));
      if (result.status === "success" && result.bytesPerSecond !== null) {
        setExportedTo(
          `下载测速 ${formatRate(result.bytesPerSecond)}（${formatBytes(result.bytesRead ?? 0)}）`,
        );
      } else if (result.status === "timeout") {
        setError("下载测速超时");
      } else {
        setError("下载测速失败");
      }
    } catch (failure: unknown) {
      setNodeSpeeds((current) => {
        const next = { ...current };
        delete next[selectedNode.id];
        return next;
      });
      setError(describeFailure(failure));
    } finally {
      setTestingSpeed(false);
    }
  }, [settings?.speedTestUrl, status]);

  const resetSubscriptionForm = useCallback(() => {
    setEditingSubscriptionId(null);
    setSubscriptionName("");
    setSubscriptionUrl("");
    setSubscriptionInterval("60");
    setSubscriptionAutoUpdate(true);
    setSubscriptionEnabled(true);
    setSubscriptionUserAgent("");
    setSubscriptionSubconverter("");
    setSubscriptionInclude("");
    setSubscriptionExclude("");
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
              excludeKeywords: subscriptionExclude,
              includeKeywords: subscriptionInclude,
              name,
              updateIntervalMinutes,
              url,
              userAgent:
                subscriptionUserAgent.trim() === ""
                  ? null
                  : subscriptionUserAgent.trim(),
              subconverterUrl:
                subscriptionSubconverter.trim() === ""
                  ? null
                  : subscriptionSubconverter.trim(),
            })
          : await updateSubscription({
              autoUpdate: subscriptionAutoUpdate,
              enabled: subscriptionEnabled,
              excludeKeywords: subscriptionExclude,
              id: editingSubscriptionId,
              includeKeywords: subscriptionInclude,
              name,
              updateIntervalMinutes,
              url: url === "" ? null : url,
              userAgent:
                subscriptionUserAgent.trim() === ""
                  ? null
                  : subscriptionUserAgent.trim(),
              subconverterUrl:
                subscriptionSubconverter.trim() === ""
                  ? null
                  : subscriptionSubconverter.trim(),
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
    subscriptionExclude,
    subscriptionInclude,
    subscriptionInterval,
    subscriptionName,
    subscriptionUrl,
    subscriptionUserAgent,
    subscriptionSubconverter,
    syncNodes,
  ]);

  const onEditSubscription = useCallback((item: SubscriptionSummary) => {
    setEditingSubscriptionId(item.id);
    setSubscriptionName(item.name);
    setSubscriptionUrl("");
    setSubscriptionInterval(String(item.updateIntervalMinutes));
    setSubscriptionAutoUpdate(item.autoUpdate);
    setSubscriptionEnabled(item.enabled);
    setSubscriptionUserAgent(item.userAgent ?? "");
    setSubscriptionSubconverter(item.subconverterUrl ?? "");
    setSubscriptionInclude(item.includeKeywords);
    setSubscriptionExclude(item.excludeKeywords);
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

  /// v2rayN's "重新加载" menu item: re-reads nodes, subscriptions, and the
  /// session status without touching the running Core.
  const onReload = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const [nextNodes, nextSubscriptions, nextStatus] = await Promise.all([
        loadNodes(),
        loadSubscriptions(),
        loadSessionStatus(),
      ]);
      setNodes(nextNodes);
      setSubscriptions(nextSubscriptions);
      setStatus(nextStatus);
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

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

  useEffect(() => {
    if (settings === null) {
      return undefined;
    }
    let cancelled = false;
    let useWindowFallback = true;
    const runAction = (action: "connect" | "previous" | "next") => {
      if (busy) {
        return;
      }
      if (action === "connect") {
        if (connected) {
          void run(disconnectSession);
        } else if (node !== null && systemProxyStartup === "clean") {
          void run(connectSession);
        }
        return;
      }
      void onStepNode(action === "previous" ? -1 : 1);
    };

    void syncGlobalHotkeys(
      {
        connect: settings.hotkeyConnect,
        next: settings.hotkeyNext,
        previous: settings.hotkeyPrevious,
      },
      runAction,
    ).then((registered) => {
      if (!cancelled) {
        useWindowFallback = !registered;
      }
    });

    const onKey = (event: KeyboardEvent) => {
      if (!useWindowFallback) {
        return;
      }
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.tagName === "SELECT" ||
          target.isContentEditable)
      ) {
        return;
      }
      if (busy) {
        return;
      }
      if (matchesHotkey(event, settings.hotkeyConnect)) {
        event.preventDefault();
        runAction("connect");
        return;
      }
      if (matchesHotkey(event, settings.hotkeyPrevious)) {
        event.preventDefault();
        runAction("previous");
        return;
      }
      if (matchesHotkey(event, settings.hotkeyNext)) {
        event.preventDefault();
        runAction("next");
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      cancelled = true;
      document.removeEventListener("keydown", onKey);
      void clearGlobalHotkeys();
    };
  }, [
    settings,
    busy,
    connected,
    node,
    systemProxyStartup,
    onStepNode,
    run,
  ]);

  const nodeGroupNames = new Map(
    nodeGroups.map((group) => [group.id, group.name]),
  );
  const visibleNodes = nodes.filter((candidate) => {
    if (nodeGroupFilter === "ungrouped" && candidate.groupId !== null) {
      return false;
    }
    if (
      nodeGroupFilter !== "all" &&
      nodeGroupFilter !== "ungrouped" &&
      candidate.groupId !== nodeGroupFilter
    ) {
      return false;
    }
    const query = nodeQuery.trim().toLowerCase();
    if (query === "") {
      return true;
    }
    const groupName =
      candidate.groupId === null
        ? ""
        : (nodeGroupNames.get(candidate.groupId) ?? "");
    return `${candidate.name} ${candidate.server} ${candidate.protocol} ${groupName}`
      .toLowerCase()
      .includes(query);
  });
  const inspected =
    nodes.find((candidate) => candidate.id === inspectedId) ??
    node ??
    visibleNodes[0] ??
    null;

  return (
    <main className="app-shell" data-layout={layout}>
      <MenuBar
        busy={busy}
        connected={connected}
        t={t}
        onAddServer={(protocol) => openCreateForm(protocol)}
        onOpenImport={() => setDialog("import")}
        onOpenCreate={() => openCreateForm()}
        onOpenSubscriptions={() => setDialog("subscriptions")}
        onRefreshSubscriptions={() => void onRefreshAllSubscriptions()}
        onOpenSettings={() => setDialog("settings")}
        onOpenRouting={() => setDialog("routing")}
        onOpenDns={() => setDialog("dns")}
        onOpenGeo={() => void onOpenGeo()}
        onCheckUpdate={() => void onCheckUpdate()}
        onCheckCoreUpdate={() => void onCheckCoreUpdate()}
        onOpenAbout={() => setDialog("about")}
        onReload={() => void onReload()}
        onClearTraffic={() => void onClearTraffic()}
        onExportAllLinks={() =>
          void onExportNodeLinks(nodes.map((candidate) => candidate.id))
        }
        onExportPreferences={() => void onExportPreferences()}
        onImportPreferences={() => void onImportPreferences()}
        onExportProfile={() => void onExportProfile()}
        onImportProfile={() => void onImportProfile()}
        onPreviousNode={() => void onStepNode(-1)}
        onNextNode={() => void onStepNode(1)}
        onConnect={() => void run(connectSession)}
        onDisconnect={() => void run(disconnectSession)}
        canConnect={node !== null && systemProxyStartup === "clean"}
        canStepNode={nodes.filter((candidate) => candidate.enabled).length >= 2}
        layout={layout}
        onLayout={setLayout}
        theme={theme}
        onTheme={setTheme}
        msgVisible={msgVisible}
        onToggleMsg={() => setMsgVisible((current) => !current)}
      />

      <div className="workspace">
        {systemProxyStartup === "restoreRequired" && (
          <section className="notice" aria-label={t("系统代理恢复")}>
            <p>
              {t("检测到上次异常退出留下的系统代理设置。恢复原设置可以避免系统继续指向已经停止的本地代理。")}
            </p>
            <div className="actions">
              <button
                type="button"
                disabled={busy}
                onClick={() =>
                  void resolveSystemProxyStartup(recoverSystemProxy)
                }
              >
                {t("恢复原设置")}
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() =>
                  void resolveSystemProxyStartup(dismissSystemProxyRecovery)
                }
              >
                {t("保留当前设置")}
              </button>
            </div>
          </section>
        )}

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

        {!noticeDismissed && (
          <div className="notice" role="note">
            <p>
              {t("这是")}<strong>{t("未签名")}</strong>版本：macOS Gatekeeper 与 Windows
              SmartScreen 会在首次打开时提示，需要你手动确认后才能运行。
            </p>
            <p>{platform ? TUN_NOTICE[platform.tunAvailability] : ""}</p>
            <div className="actions">
              <button type="button" onClick={dismissNotice}>
                {t("我知道了")}
              </button>
            </div>
          </div>
        )}

        <div className={`main-split layout-${layout}`}>
          {(layout !== "tab" || mainTab === "profiles") && (
            <section className="profiles-pane">
              <nav className="group-rail" aria-label={t("节点分组筛选")}>
                <button
                  type="button"
                  className={nodeGroupFilter === "all" ? "active" : undefined}
                  onClick={() => setNodeGroupFilter("all")}
                >
                  {t("全部")}
                </button>
                <button
                  type="button"
                  className={
                    nodeGroupFilter === "ungrouped" ? "active" : undefined
                  }
                  onClick={() => setNodeGroupFilter("ungrouped")}
                >
                  {t("未分组")}
                </button>
                {nodeGroups.map((group) => (
                  <button
                    key={group.id}
                    type="button"
                    className={
                      nodeGroupFilter === group.id ? "active" : undefined
                    }
                    onClick={() => setNodeGroupFilter(group.id)}
                  >
                    {(() => {
                      const badge = nodeGroupStrategyBadge(group.strategy, t);
                      return badge === ""
                        ? group.name
                        : `${group.name} · ${badge}`;
                    })()}
                  </button>
                ))}
              </nav>

              <div className="profiles-toolbar">
                <input
                  className="toolbar-search"
                  aria-label={t("搜索节点名称 / 服务器 / 协议")}
                  placeholder={t("搜索节点名称 / 服务器 / 协议")}
                  value={nodeQuery}
                  onChange={(event) => setNodeQuery(event.target.value)}
                />
                <div className="url-test">
                  <label>
                    {t("URL 测试地址")}
                    <input
                      aria-label={t("URL 测试地址")}
                      value={urlTestAddress}
                      disabled={busy || nodeTestInProgress}
                      onChange={(event) => setUrlTestAddress(event.target.value)}
                      onBlur={() => {
                        const address =
                          urlTestAddress.trim() || DEFAULT_URL_TEST_ADDRESS;
                        setUrlTestAddress(address);
                        if (
                          settings !== null &&
                          address !== settings.urlTestAddress
                        ) {
                          void onChangeSettings({ urlTestAddress: address });
                        }
                      }}
                    />
                  </label>
                  <button
                    type="button"
                    disabled={
                      busy || nodeTestInProgress || !connected || node === null
                    }
                    onClick={() => void onTestUrl()}
                  >
                    {t("URL 测试")}
                  </button>
                  <button
                    type="button"
                    disabled={
                      busy || nodeTestInProgress || !connected || node === null
                    }
                    onClick={() => void onSpeedTest()}
                  >
                    {t("下载测速")}
                  </button>
                </div>
                {testingAllNodes ? (
                  <button type="button" onClick={onCancelNodeTests}>
                    {t("取消测速")}
                  </button>
                ) : (
                  <button
                    type="button"
                    className="primary"
                    disabled={
                      busy ||
                      nodeTestInProgress ||
                      !nodes.some((candidate) => candidate.enabled)
                    }
                    onClick={() => void onTestAllNodes()}
                  >
                    {t("全部测速")}
                  </button>
                )}
                <button
                  type="button"
                  disabled={busy || nodes.length < 2}
                  onClick={() => void onSortNodesByLatency()}
                >
                  {t("按延迟排序")}
                </button>
                <button
                  type="button"
                  disabled={busy || lowestLatencyNodeId(nodes) === null}
                  onClick={() => void onSelectLowestLatency()}
                >
                  {t("选择最低延迟")}
                </button>
              </div>

              <div className="node-workspace">
                <div className="node-panel">

                {nodes.length === 0 ? (
                  <p className="hint">{t("尚未导入节点")}</p>
                ) : visibleNodes.length === 0 ? (
                  <p className="hint">{t("当前分组没有节点")}</p>
                ) : (
                  <table className="node-list" aria-label={t("节点列表")}>
                    <thead>
                      <tr>
                        <th className="node-check">
                          <input
                            type="checkbox"
                            aria-label={t("全选节点")}
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
                        <th className="node-index">{t("序号")}</th>
                        <th>{t("名称")}</th>
                        <th>{t("协议")}</th>
                        <th>{t("传输")}</th>
                        <th>TLS</th>
                        <th>{t("分组")}</th>
                        <th>{t("服务器")}</th>
                        <th>{t("延迟")}</th>
                        <th>{t("速度")}</th>
                        <th>{t("今日上传")}</th>
                        <th>{t("今日下载")}</th>
                        <th>{t("总上传")}</th>
                        <th>{t("总下载")}</th>
                        <th>{t("操作")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {visibleNodes.map((candidate, rowNumber) => {
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
                        const liveMs =
                          testResult?.status === "success"
                            ? testResult.latencyMs
                            : candidate.latencyMs;
                        const quality =
                          testResult?.status === "timeout" ||
                          testResult?.status === "failed" ||
                          (testResult === undefined &&
                            candidate.latencyMs === null &&
                            candidate.lastTestedAt !== null)
                            ? "fail"
                            : liveMs === null
                              ? null
                              : latencyQuality(liveMs);
                        const inspectedRow = inspected?.id === candidate.id;
                        return (
                          <tr
                            key={candidate.id}
                            className={[
                              selected ? "active-node" : "",
                              inspectedRow ? "inspected-node" : "",
                              candidate.enabled ? "" : "disabled-node",
                            ]
                              .filter(Boolean)
                              .join(" ") || undefined}
                            onClick={() => setInspectedId(candidate.id)}
                            onContextMenu={(event) => openNodeMenu(event, candidate.id)}
                            onDoubleClick={() => {
                              if (
                                !busy &&
                                !selected &&
                                !nodeTestInProgress &&
                                candidate.enabled
                              ) {
                                void run(() => switchNode(candidate.id));
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
                            <td className="node-index">{rowNumber + 1}</td>
                            <td>
                              <span
                                className={
                                  candidate.enabled
                                    ? "node-name"
                                    : "node-name node-disabled"
                                }
                              >
                                <span className="flag">{regionFlag(candidate.name)}</span>
                                {candidate.name}
                                {selected ? (
                                  <em className="pill active">{t("当前")}</em>
                                ) : null}
                                {!candidate.enabled ? (
                                  <em className="pill">{t("已禁用")}</em>
                                ) : null}
                              </span>
                            </td>
                            <td>{candidate.protocol}</td>
                            <td>{candidate.transport}</td>
                            <td>{candidate.tls ?? "—"}</td>
                            <td>{
                              candidate.groupId === null
                                ? "未分组"
                                : (nodeGroupNames.get(candidate.groupId) ?? "未知分组")
                            }</td>
                            <td>{`${candidate.server}:${candidate.port}`}</td>
                            <td>
                              <span className="latency">
                                {latency}
                                {quality === "good" ? (
                                  <em className="pill good">{t("优秀")}</em>
                                ) : null}
                                {quality === "ok" ? (
                                  <em className="pill ok">{t("一般")}</em>
                                ) : null}
                                {quality === "bad" ? (
                                  <em className="pill bad">{t("较差")}</em>
                                ) : null}
                                {quality === "fail" && latency !== "失败" ? (
                                  <em className="pill fail">{t("失败")}</em>
                                ) : null}
                              </span>
                            </td>
                            <td>
                              {(() => {
                                const speed = nodeSpeeds[candidate.id];
                                if (speed?.status === "testing") {
                                  return t("测试中");
                                }
                                if (
                                  speed?.status === "success" &&
                                  speed.bytesPerSecond !== null &&
                                  speed.bytesPerSecond !== undefined
                                ) {
                                  return formatRate(speed.bytesPerSecond);
                                }
                                if (speed?.status === "timeout") {
                                  return t("超时");
                                }
                                if (speed?.status === "failed") {
                                  return t("失败");
                                }
                                return connected && selected
                                  ? formatRate(traffic.downloadBytesPerSecond)
                                  : "—";
                              })()}
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
                            disabled={
                              busy || selected || !target.enabled
                            }
                            onClick={act(() => void run(() => switchNode(target.id)))}
                          >
                            {t("设为活动")}
                          </button>
                        </li>
                        <li>
                          <button
                            type="button"
                            role="menuitem"
                            disabled={busy || connected}
                            onClick={act(() =>
                              void run(async () => {
                                setNodes(
                                  await setNodeEnabled(target.id, !target.enabled),
                                );
                                setStatus(await loadSessionStatus());
                              }),
                            )}
                          >
                            {target.enabled ? t("禁用节点") : t("启用节点")}
                          </button>
                        </li>
                        <li>
                          <button
                            type="button"
                            role="menuitem"
                            disabled={
                              busy ||
                              (batch.length === 1
                                ? !target.enabled
                                : !batch.some((id) =>
                                    nodes.some(
                                      (candidate) =>
                                        candidate.id === id && candidate.enabled,
                                    ),
                                  ))
                            }
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
                            {t("克隆所选")}
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
                            {t("显示二维码")}
                          </button>
                        </li>
                        <li>
                          <button
                            type="button"
                            role="menuitem"
                            disabled={busy}
                            onClick={act(() => onGroupNode(target))}
                          >
                            {t("设置分组")}
                          </button>
                        </li>
                        <li>
                          <button
                            type="button"
                            role="menuitem"
                            disabled={busy || !reorderable || index === 0}
                            onClick={act(() => void onMoveNode(target.id, "up"))}
                          >
                            {t("上移")}
                          </button>
                        </li>
                        <li>
                          <button
                            type="button"
                            role="menuitem"
                            disabled={busy || !reorderable || index === nodes.length - 1}
                            onClick={act(() => void onMoveNode(target.id, "down"))}
                          >
                            {t("下移")}
                          </button>
                        </li>
                        <li>
                          <button
                            type="button"
                            role="menuitem"
                            disabled={busy || !reorderable || nodes.length < 2}
                            onClick={act(() => void onSortNodesByLatency())}
                          >
                            {t("按延迟排序")}
                          </button>
                        </li>
                        {target.deletable && (
                          <li>
                            <button
                              type="button"
                              role="menuitem"
                              disabled={busy || connected}
                              onClick={act(() => void onEditNode(target))}
                            >
                              {t("编辑")}
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
                            {t("移除重复")}
                          </button>
                        </li>
                      </ul>
                    );
                  })()}

                {groupingNodeId !== null && (
                  <div className="settings-form" aria-label={t("设置节点分组")}>
                    <label>
                      {t("分组名称")}
                      <input
                        aria-label={t("节点分组")}
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
                    <label>
                      {t("分组策略")}
                      <select
                        aria-label={t("分组策略")}
                        value={nodeGroupStrategy}
                        disabled={busy}
                        onChange={(event) =>
                          setNodeGroupStrategy(
                            event.target.value as NodeGroupStrategy,
                          )
                        }
                      >
                        <option value="select">{t("手动选择")}</option>
                        <option value="urlTest">{t("URL-TEST 自动测速")}</option>
                        <option value="fallback">{t("故障转移")}</option>
                        <option value="loadBalance">{t("负载均衡")}</option>
                      </select>
                    </label>
                    <p className="hint">
                      {nodeGroupStrategy === "urlTest"
                        ? t("该分组连接时由 Core 测速选节点")
                        : nodeGroupStrategy === "fallback"
                          ? t("按列表顺序尝试节点，失败则切换下一个")
                          : nodeGroupStrategy === "loadBalance"
                            ? t("连接时在组成员之间分配流量")
                            : t("连接时使用你在列表中选中的节点")}
                    </p>
                    <div className="actions">
                      <button type="button" disabled={busy} onClick={() => void onSaveNodeGroup()}>
                        {t("保存分组")}
                      </button>
                      <button type="button" disabled={busy} onClick={resetNodeGroupForm}>
                        {t("取消")}
                      </button>
                    </div>
                  </div>
                )}

                </div>
                <aside className="detail-panel" aria-label={t("节点详情")}>
                  <header>
                    <h2>{t("节点详情")}</h2>
                  </header>
                  {inspected === null ? (
                    <p className="hint">{t("尚未导入节点")}</p>
                  ) : (
                    <>
                      <div className="detail-title">
                        <span className="flag">{regionFlag(inspected.name)}</span>
                        <strong>{inspected.name}</strong>
                        {inspected.id === node?.id ? (
                          <em className="pill active">{t("当前使用")}</em>
                        ) : null}
                      </div>
                      <dl className="detail-list">
                        <div>
                          <dt>{t("协议")}</dt>
                          <dd>{inspected.protocol}</dd>
                        </div>
                        <div>
                          <dt>{t("服务器")}</dt>
                          <dd>{inspected.server}</dd>
                        </div>
                        <div>
                          <dt>{t("端口")}</dt>
                          <dd>{inspected.port}</dd>
                        </div>
                        <div>
                          <dt>{t("传输")}</dt>
                          <dd>{inspected.transport}</dd>
                        </div>
                        <div>
                          <dt>TLS</dt>
                          <dd>{inspected.tls ?? "—"}</dd>
                        </div>
                        <div>
                          <dt>{t("分组")}</dt>
                          <dd>
                            {inspected.groupId === null
                              ? t("未分组")
                              : (nodeGroupNames.get(inspected.groupId) ?? t("未知分组"))}
                          </dd>
                        </div>
                        <div>
                          <dt>{t("延迟")}</dt>
                          <dd>
                            {inspected.latencyMs === null
                              ? inspected.lastTestedAt === null
                                ? "—"
                                : t("失败")
                              : `${inspected.latencyMs} ms`}
                          </dd>
                        </div>
                        <div>
                          <dt>{t("最后测试")}</dt>
                          <dd>
                            {inspected.lastTestedAt === null
                              ? t("尚未测试")
                              : formatClock(inspected.lastTestedAt)}
                          </dd>
                        </div>
                      </dl>
                      <div className="detail-actions">
                        <button
                          type="button"
                          className="primary"
                          disabled={
                            busy ||
                            inspected.id === node?.id ||
                            nodeTestInProgress ||
                            !inspected.enabled
                          }
                          onClick={() => void run(() => switchNode(inspected.id))}
                        >
                          {t("设为当前节点")}
                        </button>
                        <button
                          type="button"
                          disabled={busy || nodeTestInProgress}
                          onClick={() => void onTestNode(inspected.id)}
                        >
                          {t("测试延迟")}
                        </button>
                      </div>
                    </>
                  )}
                </aside>
              </div>
            </section>
          )}
          {msgVisible && (layout !== "tab" || mainTab === "msg") && (
            <MsgView
              busy={busy}
              logs={logs}
              logLevel={logLevel}
              logSource={logSource}
              t={t}
              onLogLevel={setLogLevel}
              onLogSource={setLogSource}
              onRefresh={() => void refreshLogs()}
              onClear={() => void onClearLogs()}
              onExport={() => void onExport()}
            />
          )}
        </div>
        {layout === "tab" && (
          <div className="main-tabs">
            <button
              type="button"
              className={mainTab === "profiles" ? "active" : undefined}
              onClick={() => setMainTab("profiles")}
            >
              {t("服务器")}
            </button>
            <button
              type="button"
              className={mainTab === "msg" ? "active" : undefined}
              onClick={() => setMainTab("msg")}
            >
              {t("消息")}
            </button>
          </div>
        )}
      </div>

      <StatusBar
        busy={busy}
        connected={connected}
        locale={locale}
        platform={platform}
        settings={settings}
        status={status}
        traffic={traffic}
        t={t}
        onChangeSettings={onChangeSettings}
        onRoutingMode={(mode) => void run(() => setRoutingMode(mode))}
      />

      {update !== null && (
          <div className="dialog-backdrop" onClick={() => setUpdate(null)}>
            <div
              className="dialog qr-dialog"
              role="dialog"
              aria-label={t("检查更新结果")}
              onClick={(event) => event.stopPropagation()}
            >
              <header className="dialog-head">
                <strong>{t("检查更新")}</strong>
                <button type="button" onClick={() => setUpdate(null)}>
                  {t("关闭")}
                </button>
              </header>
              <p>
                {update.updateAvailable
                  ? `有新版本 ${update.latest}，当前 ${update.current}`
                  : `已是最新版本 ${update.current}`}
              </p>
              {/* A link rather than an in-app download: the artifacts are
                  unsigned, so the user has to see what they are fetching. */}
              <p className="hint">{update.url}</p>
            </div>
          </div>
        )}
      {coreUpdate !== null && (
          <div className="dialog-backdrop" onClick={() => setCoreUpdate(null)}>
            <div
              className="dialog qr-dialog"
              role="dialog"
              aria-label={t("检查 Core 更新结果")}
              onClick={(event) => event.stopPropagation()}
            >
              <header className="dialog-head">
                <strong>{t("检查 Core 更新")}</strong>
                <button type="button" onClick={() => setCoreUpdate(null)}>
                  {t("关闭")}
                </button>
              </header>
              <p>{formatCoreVersionLine(coreUpdate.singBox)}</p>
              <p className="hint">{formatInstalledCoreLine("sing-box", coreUpdate.install.singBox)}</p>
              <p className="hint">{coreUpdate.singBox.url}</p>
              <p>{formatCoreVersionLine(coreUpdate.xray)}</p>
              <p className="hint">{formatInstalledCoreLine("Xray", coreUpdate.install.xray)}</p>
              <p className="hint">{coreUpdate.xray.url}</p>
              <p className="hint">
                {t("Core 更新说明")}
              </p>
              <div className="actions">
                <button
                  type="button"
                  disabled={busy || connected || !coreUpdate.singBox.updateAvailable}
                  onClick={() => void onDownloadCore("sing-box")}
                >
                  {t("下载 sing-box")}
                </button>
                <button
                  type="button"
                  disabled={busy || connected || !coreUpdate.xray.updateAvailable}
                  onClick={() => void onDownloadCore("xray")}
                >
                  {t("下载 Xray")}
                </button>
              </div>
              {connected ? (
                <p className="hint">{t("请先断开连接再更新 Core")}</p>
              ) : null}
            </div>
          </div>
        )}
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
                  {t("关闭")}
                </button>
              </header>
              {/* The markup comes from the Rust renderer, never from a node's
                  own fields, so there is nothing here a node name could inject. */}
              <div
                className="qr-code"
                dangerouslySetInnerHTML={{ __html: qrCode.svg }}
              />
              <p className="hint">
                {t("扫描即导入该节点。二维码包含凭据，请勿分享给他人。")}
              </p>
            </div>
          </div>
        )}
      <Dialog
        hidden={dialog !== "import"}
        title={t("导入节点")}
        ariaLabel={t("导入节点")}
        onClose={() => setDialog(null)}
        wide
      >
        <h2>{t("导入节点")}</h2>

        <textarea
          aria-label={t("分享链接")}
          rows={3}
          value={uri}
          disabled={busy || connected}
          placeholder="vless:// vmess:// trojan:// ss:// hysteria2:// tuic://"
          onChange={(event) => setUri(event.target.value)}
        />

        <div className="actions">
          <button
            type="button"
            disabled={busy || connected}
            onClick={() => void onImport()}
          >
            {t("导入")}
          </button>
        </div>

        <h2>{t("批量导入")}</h2>

        <p className="hint">
          {t("可粘贴多行分享链接，或整体 Base64 的订阅正文。批量导入不会改变当前选中的节点。")}
        </p>

        <textarea
          aria-label={t("批量节点列表")}
          rows={4}
          value={bulkText}
          disabled={busy || connected}
          placeholder={t("每行一个链接，或粘贴 Base64 订阅正文")}
          onChange={(event) => setBulkText(event.target.value)}
        />

        <div className="actions">
          <button
            type="button"
            disabled={busy || connected}
            onClick={() => void runBulkImport(bulkText)}
          >
            {t("批量导入")}
          </button>
          <label className="file-import">
            {t("从文件导入")}
            <input
              aria-label={t("从文件导入节点")}
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
            {t("扫描二维码图片")}
            <input
              aria-label={t("从二维码图片导入节点")}
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
          <div className="bulk-report" role="status" aria-label={t("批量导入结果")}>
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

      </Dialog>

      <Dialog
        hidden={dialog !== "create"}
        title={editingNodeId === null ? t("手动创建节点") : t("编辑节点")}
        ariaLabel={editingNodeId === null ? t("手动创建节点") : t("编辑节点")}
        onClose={() => {
          setDialog(null);
          resetNodeForm();
        }}
        wide
      >
        <h2>{editingNodeId === null ? t("手动创建节点") : t("编辑节点")}</h2>

        <div
          className="settings-form"
          aria-label={editingNodeId === null ? t("手动创建节点") : t("编辑节点")}
        >
          <label>
            {t("协议")}
            <select
              aria-label={t("节点协议")}
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
              <option value="socks">SOCKS5</option>
              <option value="http">HTTP</option>
              <option value="hysteria2">Hysteria2</option>
              <option value="tuic">TUIC</option>
              <option value="wireguard">WireGuard</option>
              <option value="anytls">AnyTLS</option>
              <option value="naive">Naive</option>
              <option value="custom">Custom</option>
            </select>
          </label>

          <label>
            {t("名称")}
            <input
              aria-label={t("新建节点名称")}
              value={createForm.name}
              disabled={busy || connected}
              onChange={(event) => updateCreateForm({ name: event.target.value })}
            />
          </label>

          {createForm.protocol !== "custom" && (
            <>
          <label>
            {t("服务器")}
            <input
              aria-label={t("新建节点服务器")}
              value={createForm.server}
              disabled={busy || connected}
              onChange={(event) =>
                updateCreateForm({ server: event.target.value })
              }
            />
          </label>

          <label>
            {t("端口")}
            <input
              aria-label={t("新建节点端口")}
              inputMode="numeric"
              value={createForm.port}
              disabled={busy || connected}
              onChange={(event) => updateCreateForm({ port: event.target.value })}
            />
          </label>
            </>
          )}

          {createForm.protocol === "custom" && (
            <>
              <p className="form-hint">
                {t("自定义节点使用完整 Core JSON，忽略 Mux、Fragment、DNS 与路由设置；请自行保证 SOCKS/HTTP 端口与设置页一致。")}
              </p>
              <label>
                {t("Core 类型")}
                <select
                  aria-label={t("自定义 Core 类型")}
                  value={createForm.customCore}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({
                      customCore: event.target.value as "sing-box" | "xray",
                    })
                  }
                >
                  <option value="sing-box">sing-box</option>
                  <option value="xray">Xray</option>
                </select>
              </label>
              <label>
                {t("Core JSON 配置")}
                <textarea
                  aria-label={t("Core JSON 配置")}
                  rows={16}
                  spellCheck={false}
                  value={createForm.customDocument}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ customDocument: event.target.value })
                  }
                />
              </label>
            </>
          )}

          {(createForm.protocol === "vless" ||
            createForm.protocol === "vmess" ||
            createForm.protocol === "tuic") && (
            <label>
              UUID
              <input
                aria-label={t("节点 UUID")}
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
                placeholder={t("留空表示不使用")}
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
                {t("加密方式")}
                <select
                  aria-label={t("VMess 加密方式")}
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
              {t("加密方式")}
              <select
                aria-label={t("Shadowsocks 加密方式")}
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

          {(createForm.protocol === "socks" ||
            createForm.protocol === "http" ||
            createForm.protocol === "naive") && (
            <label>
              {t("用户名")}
              <input
                aria-label={t("代理用户名")}
                placeholder={t("留空表示不使用")}
                value={createForm.username}
                disabled={busy || connected}
                onChange={(event) =>
                  updateCreateForm({ username: event.target.value })
                }
              />
            </label>
          )}

          {(createForm.protocol === "trojan" ||
            createForm.protocol === "shadowsocks" ||
            createForm.protocol === "socks" ||
            createForm.protocol === "http" ||
            createForm.protocol === "anytls" ||
            createForm.protocol === "naive") && (
            <label>
              {t("密码")}
              <input
                aria-label={
                  createForm.protocol === "anytls"
                    ? t("AnyTLS 密码")
                    : t("节点密码")
                }
                type="password"
                placeholder={
                  createForm.protocol === "socks" ||
                  createForm.protocol === "http" ||
                  createForm.protocol === "naive"
                    ? t("留空表示不使用")
                    : undefined
                }
                value={createForm.password}
                disabled={busy || connected}
                onChange={(event) =>
                  updateCreateForm({ password: event.target.value })
                }
              />
            </label>
          )}

          {createForm.protocol === "naive" && (
            <>
              <label>
                <input
                  aria-label={t("启用 QUIC")}
                  type="checkbox"
                  checked={createForm.quic}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ quic: event.target.checked })
                  }
                />
                {t("启用 QUIC")}
              </label>
              {createForm.quic && (
                <label>
                  {t("拥塞控制")}
                  <select
                    aria-label={t("Naive 拥塞控制")}
                    value={createForm.quicCongestionControl}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({
                        quicCongestionControl: event.target
                          .value as ManualNodeForm["quicCongestionControl"],
                      })
                    }
                  >
                    <option value="">{t("默认")}</option>
                    <option value="bbr">bbr</option>
                    <option value="bbr2">bbr2</option>
                    <option value="cubic">cubic</option>
                    <option value="reno">reno</option>
                  </select>
                </label>
              )}
            </>
          )}

          {createForm.protocol === "hysteria2" && (
            <>
              <label>
                {t("认证密码")}
                <input
                  aria-label={t("Hysteria2 认证密码")}
                  type="password"
                  placeholder={t("留空表示不使用")}
                  value={createForm.authentication}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ authentication: event.target.value })
                  }
                />
              </label>
              <label>
                <input
                  aria-label={t("启用混淆")}
                  type="checkbox"
                  checked={createForm.obfsEnabled}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ obfsEnabled: event.target.checked })
                  }
                />
                {t("启用混淆")}
              </label>
              {createForm.obfsEnabled && (
                <>
                  <label>
                    {t("混淆方式")}
                    <select
                      aria-label={t("混淆方式")}
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
                    {t("混淆密码")}
                    <input
                      aria-label={t("混淆密码")}
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

          {createForm.protocol === "tuic" && (
            <>
              <label>
                {t("TUIC 密码")}
                <input
                  aria-label={t("TUIC 密码")}
                  type="password"
                  placeholder={t("留空表示不使用")}
                  value={createForm.password}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ password: event.target.value })
                  }
                />
              </label>
              <label>
                {t("拥塞控制")}
                <select
                  aria-label={t("拥塞控制")}
                  value={createForm.congestionControl}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({
                      congestionControl: event.target.value as
                        | ""
                        | "cubic"
                        | "new_reno"
                        | "bbr",
                    })
                  }
                >
                  <option value="">{t("默认")}</option>
                  <option value="cubic">cubic</option>
                  <option value="new_reno">new_reno</option>
                  <option value="bbr">bbr</option>
                </select>
              </label>
              <label>
                {t("UDP 中继模式")}
                <select
                  aria-label={t("UDP 中继模式")}
                  value={createForm.udpRelayMode}
                  disabled={busy || connected || createForm.udpOverStream}
                  onChange={(event) =>
                    updateCreateForm({
                      udpRelayMode: event.target.value as
                        | ""
                        | "native"
                        | "quic",
                    })
                  }
                >
                  <option value="">{t("默认")}</option>
                  <option value="native">native</option>
                  <option value="quic">quic</option>
                </select>
              </label>
              <label className="checkbox-label">
                <input
                  aria-label={t("UDP over stream")}
                  type="checkbox"
                  checked={createForm.udpOverStream}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({
                      udpOverStream: event.target.checked,
                      udpRelayMode: event.target.checked
                        ? ""
                        : createForm.udpRelayMode,
                    })
                  }
                />
                UDP over stream
              </label>
              <label className="checkbox-label">
                <input
                  aria-label={t("0-RTT 握手")}
                  type="checkbox"
                  checked={createForm.zeroRttHandshake}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({
                      zeroRttHandshake: event.target.checked,
                    })
                  }
                />
                {t("0-RTT 握手")}
              </label>
            </>
          )}

          {createForm.protocol === "wireguard" && (
            <>
              <label>
                {t("私钥")}
                <input
                  aria-label={t("WireGuard 私钥")}
                  type="password"
                  value={createForm.privateKey}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ privateKey: event.target.value })
                  }
                />
              </label>
              <label>
                {t("对端公钥")}
                <input
                  aria-label={t("WireGuard 对端公钥")}
                  value={createForm.peerPublicKey}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ peerPublicKey: event.target.value })
                  }
                />
              </label>
              <label>
                {t("预共享密钥")}
                <input
                  aria-label={t("WireGuard 预共享密钥")}
                  type="password"
                  placeholder={t("留空表示不使用")}
                  value={createForm.preSharedKey}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ preSharedKey: event.target.value })
                  }
                />
              </label>
              <label>
                {t("本地地址")}
                <input
                  aria-label={t("WireGuard 本地地址")}
                  placeholder={t("逗号分隔，如 10.0.0.2/32")}
                  value={createForm.localAddress}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ localAddress: event.target.value })
                  }
                />
              </label>
              <label>
                MTU
                <input
                  aria-label="WireGuard MTU"
                  inputMode="numeric"
                  placeholder={t("留空表示不使用")}
                  value={createForm.mtu}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ mtu: event.target.value })
                  }
                />
              </label>
              <label>
                Reserved
                <input
                  aria-label="WireGuard reserved"
                  placeholder={t("留空表示不使用，如 1,2,3")}
                  value={createForm.reserved}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ reserved: event.target.value })
                  }
                />
              </label>
            </>
          )}

          {usesStreamTransport(createForm.protocol) && (
            <label>
              {t("传输方式")}
              <select
                aria-label={t("传输方式")}
                value={createForm.transport}
                disabled={
                  busy ||
                  connected ||
                  createForm.protocol === "shadowsocks" ||
                  createForm.protocol === "socks" ||
                  createForm.protocol === "http"
                }
                onChange={(event) =>
                  updateCreateForm({
                    transport: event.target.value as TransportKind,
                  })
                }
              >
                <option value="tcp">TCP</option>
                <option value="websocket">WebSocket</option>
                <option value="httpupgrade">HTTPUpgrade</option>
                <option value="xhttp">XHTTP</option>
                <option value="grpc">gRPC</option>
                <option value="kcp">mKCP</option>
              </select>
            </label>
          )}

          {usesStreamTransport(createForm.protocol) &&
            (createForm.transport === "websocket" ||
              createForm.transport === "httpupgrade" ||
              createForm.transport === "xhttp") && (
              <>
                <label>
                  {t("路径")}
                  <input
                    aria-label={
                      createForm.transport === "httpupgrade"
                        ? t("HTTPUpgrade 路径")
                        : createForm.transport === "xhttp"
                          ? t("XHTTP 路径")
                          : t("WebSocket 路径")
                    }
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
                    aria-label={
                      createForm.transport === "httpupgrade"
                        ? "HTTPUpgrade Host"
                        : createForm.transport === "xhttp"
                          ? "XHTTP Host"
                          : "WebSocket Host"
                    }
                    placeholder={t("留空表示不使用")}
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
            createForm.transport === "xhttp" && (
              <label>
                {t("模式")}
                <select
                  aria-label={t("XHTTP 模式")}
                  value={createForm.xhttpMode}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({
                      xhttpMode: event.target.value as XhttpMode,
                    })
                  }
                >
                  <option value="auto">auto</option>
                  <option value="packet-up">packet-up</option>
                  <option value="stream-up">stream-up</option>
                  <option value="stream-one">stream-one</option>
                </select>
              </label>
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
                  {t("模式")}
                  <select
                    aria-label={t("gRPC 模式")}
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
                    placeholder={t("留空表示不使用")}
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
            createForm.transport === "kcp" && (
              <>
                <label>
                  mtu
                  <input
                    aria-label={t("mKCP mtu")}
                    placeholder={t("留空表示使用默认")}
                    value={createForm.kcpMtu}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ kcpMtu: event.target.value })
                    }
                  />
                </label>
                <label>
                  tti
                  <input
                    aria-label={t("mKCP tti")}
                    placeholder={t("留空表示使用默认")}
                    value={createForm.kcpTti}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ kcpTti: event.target.value })
                    }
                  />
                </label>
                <label>
                  uplinkCapacity
                  <input
                    aria-label={t("mKCP uplinkCapacity")}
                    placeholder={t("留空表示使用默认")}
                    value={createForm.kcpUplinkCapacity}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ kcpUplinkCapacity: event.target.value })
                    }
                  />
                </label>
                <label>
                  downlinkCapacity
                  <input
                    aria-label={t("mKCP downlinkCapacity")}
                    placeholder={t("留空表示使用默认")}
                    value={createForm.kcpDownlinkCapacity}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({
                        kcpDownlinkCapacity: event.target.value,
                      })
                    }
                  />
                </label>
                <label>
                  {t("伪装类型")}
                  <select
                    aria-label={t("mKCP 伪装类型")}
                    value={createForm.kcpHeaderType}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({
                        kcpHeaderType: event.target.value as KcpHeaderType,
                      })
                    }
                  >
                    <option value="">{t("默认")}</option>
                    <option value="none">none</option>
                    <option value="srtp">srtp</option>
                    <option value="utp">utp</option>
                    <option value="wechat-video">wechat-video</option>
                    <option value="dtls">dtls</option>
                    <option value="wireguard">wireguard</option>
                  </select>
                </label>
                <label>
                  seed
                  <input
                    aria-label={t("mKCP seed")}
                    placeholder={t("留空表示不使用")}
                    value={createForm.kcpSeed}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ kcpSeed: event.target.value })
                    }
                  />
                </label>
                <label>
                  <input
                    aria-label={t("mKCP 拥塞控制")}
                    type="checkbox"
                    checked={createForm.kcpCongestion}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ kcpCongestion: event.target.checked })
                    }
                  />
                  {t("启用拥塞控制")}
                </label>
              </>
            )}

          {usesStreamTransport(createForm.protocol) &&
            createForm.protocol !== "shadowsocks" &&
            createForm.protocol !== "socks" && (
              <>
                <label>
                  <input
                    aria-label={t("启用 TLS")}
                    type="checkbox"
                    checked={createForm.tlsEnabled || createForm.realityEnabled}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({
                        tlsEnabled: event.target.checked,
                        realityEnabled: event.target.checked
                          ? createForm.realityEnabled
                          : false,
                      })
                    }
                  />
                  {t("启用 TLS")}
                </label>
                {createForm.protocol !== "http" && (
                  <label>
                    <input
                      aria-label={t("启用 Reality")}
                      type="checkbox"
                      checked={createForm.realityEnabled}
                      disabled={busy || connected}
                      onChange={(event) =>
                        updateCreateForm({
                          realityEnabled: event.target.checked,
                          tlsEnabled:
                            event.target.checked || createForm.tlsEnabled,
                        })
                      }
                    />
                    {t("启用 Reality")}
                  </label>
                )}
              </>
            )}

          {createForm.protocol === "anytls" && (
            <>
              <label>
                <input
                  aria-label={t("启用 TLS")}
                  type="checkbox"
                  checked={createForm.tlsEnabled || createForm.realityEnabled}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({
                      tlsEnabled: event.target.checked,
                      realityEnabled: event.target.checked
                        ? createForm.realityEnabled
                        : false,
                    })
                  }
                />
                {t("启用 TLS")}
              </label>
              <label>
                <input
                  aria-label={t("启用 Reality")}
                  type="checkbox"
                  checked={createForm.realityEnabled}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({
                      realityEnabled: event.target.checked,
                      tlsEnabled: event.target.checked || createForm.tlsEnabled,
                    })
                  }
                />
                {t("启用 Reality")}
              </label>
            </>
          )}

          {(createForm.tlsEnabled ||
            createForm.realityEnabled ||
            createForm.protocol === "hysteria2" ||
            createForm.protocol === "tuic" ||
            createForm.protocol === "anytls" ||
            createForm.protocol === "naive") &&
            createForm.protocol !== "shadowsocks" &&
            createForm.protocol !== "socks" && (
              <>
                <label>
                  SNI
                  <input
                    aria-label="TLS SNI"
                    placeholder={t("留空表示使用服务器地址")}
                    value={createForm.serverName}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ serverName: event.target.value })
                    }
                  />
                </label>
                {createForm.protocol !== "naive" && (
                  <>
                <label>
                  ALPN
                  <input
                    aria-label="TLS ALPN"
                    placeholder={t("逗号分隔，如 h2,http/1.1")}
                    value={createForm.alpn}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ alpn: event.target.value })
                    }
                  />
                </label>
                <label>
                  {t("指纹")}
                  <input
                    aria-label={t("TLS 指纹")}
                    placeholder={t("留空表示不使用")}
                    value={createForm.fingerprint}
                    disabled={busy || connected}
                    onChange={(event) =>
                      updateCreateForm({ fingerprint: event.target.value })
                    }
                  />
                </label>
                {!createForm.realityEnabled && (
                  <label>
                    pinSHA256
                    <input
                      aria-label={t("证书固定 pinSHA256")}
                      placeholder={t("留空表示不使用")}
                      value={createForm.pinnedSha256}
                      disabled={busy || connected}
                      onChange={(event) =>
                        updateCreateForm({ pinnedSha256: event.target.value })
                      }
                    />
                  </label>
                )}
                {createForm.realityEnabled ? (
                  <>
                    <label>
                      publicKey
                      <input
                        aria-label={t("Reality publicKey")}
                        value={createForm.publicKey}
                        disabled={busy || connected}
                        onChange={(event) =>
                          updateCreateForm({ publicKey: event.target.value })
                        }
                      />
                    </label>
                    <label>
                      shortId
                      <input
                        aria-label={t("Reality shortId")}
                        placeholder={t("留空表示不使用")}
                        value={createForm.shortId}
                        disabled={busy || connected}
                        onChange={(event) =>
                          updateCreateForm({ shortId: event.target.value })
                        }
                      />
                    </label>
                    <label>
                      spiderX
                      <input
                        aria-label={t("Reality spiderX")}
                        placeholder={t("留空表示不使用")}
                        value={createForm.spiderX}
                        disabled={busy || connected}
                        onChange={(event) =>
                          updateCreateForm({ spiderX: event.target.value })
                        }
                      />
                    </label>
                  </>
                ) : (
                  <label>
                    <input
                      aria-label={t("允许不安全证书")}
                      type="checkbox"
                      checked={createForm.allowInsecure}
                      disabled={busy || connected}
                      onChange={(event) =>
                        updateCreateForm({ allowInsecure: event.target.checked })
                      }
                    />
                    {t("允许不安全证书")}
                  </label>
                )}
                  </>
                )}
              </>
            )}

          <label>
            <input
              aria-label={t("启用 UDP")}
              type="checkbox"
              checked={createForm.udpEnabled}
              disabled={busy || connected}
              onChange={(event) =>
                updateCreateForm({ udpEnabled: event.target.checked })
              }
            />
            {t("启用 UDP")}
          </label>

          {createForm.protocol !== "custom" && (
            <>
              <label>
                {t("Xray Finalmask JSON")}
                <textarea
                  aria-label={t("Xray Finalmask JSON")}
                  rows={4}
                  placeholder={t("留空使用全局 Final Fragment 默认 mask")}
                  value={createForm.xrayFinalmaskJson}
                  disabled={busy || connected}
                  onChange={(event) =>
                    updateCreateForm({ xrayFinalmaskJson: event.target.value })
                  }
                />
              </label>
              <p className="hint">
                {t("仅 Xray 且在设置中启用 Final Fragment 时生效；可填 mask 条目或 {tcp:[...]} 对象。")}
              </p>
            </>
          )}

          <div className="actions">
            <button
              type="button"
              disabled={busy || connected}
              onClick={() => void onCreateNode()}
            >
              {editingNodeId === null ? t("创建节点") : t("保存节点")}
            </button>
            <button
              type="button"
              disabled={busy || connected}
              onClick={() =>
                setCreateForm(
                  blankManualNodeForm({
                    allowInsecure: settings?.defAllowInsecure,
                    fingerprint: settings?.defFingerprint,
                  }),
                )
              }
            >
              {t("重置")}
            </button>
          </div>
        </div>

      </Dialog>

      <Dialog
        hidden={dialog !== "subscriptions"}
        title={t("订阅")}
        ariaLabel={t("订阅")}
        onClose={() => setDialog(null)}
        wide
      >
        <h2>{t("订阅")}</h2>

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
            {t("全部更新")}
          </button>
        </div>

        {subscriptions.length === 0 ? (
          <p className="hint">{t("尚未添加订阅")}</p>
        ) : (
          <table className="node-list" aria-label={t("订阅列表")}>
            <thead>
              <tr>
                <th>{t("名称")}</th>
                <th>{t("节点")}</th>
                <th>{t("更新")}</th>
                <th>{t("操作")}</th>
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
                      {t("编辑")}
                    </button>
                    <button
                      type="button"
                      aria-label={`刷新 ${item.name}`}
                      disabled={
                        busy || connected || nodeTestInProgress || !item.enabled
                      }
                      onClick={() => void onRefreshSubscription(item.id)}
                    >
                      {t("刷新")}
                    </button>
                    <button
                      type="button"
                      aria-label={`删除订阅 ${item.name}`}
                      disabled={busy || connected || nodeTestInProgress}
                      onClick={() => void onDeleteSubscription(item.id)}
                    >
                      {t("删除")}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        <div className="subscription-form">
          <label>
            {t("名称")}
            <input
              aria-label={t("订阅名称")}
              value={subscriptionName}
              disabled={busy}
              onChange={(event) => setSubscriptionName(event.target.value)}
            />
          </label>
          <label>
            {t("地址")}
            <textarea
              aria-label={t("订阅地址")}
              disabled={busy}
              rows={3}
              placeholder={
                editingSubscriptionId === null
                  ? "https://\n一行一个，或用 | 分隔"
                  : t("留空则不修改")
              }
              value={subscriptionUrl}
              onChange={(event) => setSubscriptionUrl(event.target.value)}
            />
          </label>
          <label>
            {t("Subconverter 地址")}
            <input
              aria-label={t("Subconverter 地址")}
              disabled={busy}
              placeholder="http://127.0.0.1:25500/sub"
              value={subscriptionSubconverter}
              onChange={(event) =>
                setSubscriptionSubconverter(event.target.value)
              }
            />
          </label>
          <label>
            {t("更新间隔（分钟）")}
            <input
              aria-label={t("更新间隔")}
              type="number"
              min="1"
              value={subscriptionInterval}
              disabled={busy}
              onChange={(event) => setSubscriptionInterval(event.target.value)}
            />
          </label>
          <label>
            User-Agent
            <input
              aria-label={t("订阅 User-Agent")}
              placeholder={t("留空使用默认")}
              value={subscriptionUserAgent}
              disabled={busy}
              onChange={(event) => setSubscriptionUserAgent(event.target.value)}
            />
          </label>
          <label>
            {t("包含关键词")}
            <input
              aria-label={t("包含关键词")}
              placeholder={t("用 | 分隔，留空表示全部")}
              value={subscriptionInclude}
              disabled={busy}
              onChange={(event) => setSubscriptionInclude(event.target.value)}
            />
          </label>
          <label>
            {t("排除关键词")}
            <input
              aria-label={t("排除关键词")}
              placeholder={t("用 | 分隔")}
              value={subscriptionExclude}
              disabled={busy}
              onChange={(event) => setSubscriptionExclude(event.target.value)}
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
            {t("自动更新")}
          </label>
          {editingSubscriptionId !== null && (
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={subscriptionEnabled}
                disabled={busy}
                onChange={(event) => setSubscriptionEnabled(event.target.checked)}
              />
              {t("启用订阅")}
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
              {t("取消")}
            </button>
          )}
        </div>

      </Dialog>

      <Dialog
        hidden={dialog !== "routing"}
        title={t("路由规则")}
        ariaLabel={t("路由规则")}
        onClose={() => setDialog(null)}
        wide
      >
        <h2>{t("路由规则")}</h2>

        <p className="hint">
          {t("运行顺序固定为：本地安全规则 → 用户规则 → Geo 规则 → 默认出口。仅规则模式应用列表。")}
        </p>
        {routeDraft === null || status === null ? (
          <p className="hint">{t("正在读取路由设置")}</p>
        ) : (
          <>
            <div className="settings-form">
              <label>
                {t("路由方案")}
                <select
                  aria-label={t("路由方案")}
                  disabled={busy || connected}
                  value={status.routeSchemeId}
                  onChange={(event) => {
                    void (async () => {
                      setBusy(true);
                      setError(null);
                      try {
                        const next = await setRouteScheme(event.target.value);
                        setStatus(next);
                        setRouteDraft(runtimeOrderedRoute(next.route));
                        setRouteDirty(false);
                      } catch (failure: unknown) {
                        setError(describeFailure(failure));
                      } finally {
                        setBusy(false);
                      }
                    })();
                  }}
                >
                  {status.routeSchemes.map((scheme) => (
                    <option key={scheme.id} value={scheme.id}>
                      {scheme.name}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="actions">
              <button
                type="button"
                disabled={busy || connected}
                onClick={() => {
                  void (async () => {
                    const name = window.prompt(t("新路由方案名称"), t("新方案"));
                    if (name === null || name.trim() === "") {
                      return;
                    }
                    setBusy(true);
                    setError(null);
                    try {
                      const next = await createRouteScheme(name.trim());
                      setStatus(next);
                      setRouteDraft(runtimeOrderedRoute(next.route));
                      setRouteDirty(false);
                    } catch (failure: unknown) {
                      setError(describeFailure(failure));
                    } finally {
                      setBusy(false);
                    }
                  })();
                }}
              >
                {t("新建方案")}
              </button>
              <button
                type="button"
                disabled={
                  busy ||
                  connected ||
                  status.routeSchemes.length <= 1
                }
                onClick={() => {
                  void (async () => {
                    if (
                      !window.confirm(
                        t("确定删除当前路由方案？"),
                      )
                    ) {
                      return;
                    }
                    setBusy(true);
                    setError(null);
                    try {
                      const next = await deleteRouteScheme(status.routeSchemeId);
                      setStatus(next);
                      setRouteDraft(runtimeOrderedRoute(next.route));
                      setRouteDirty(false);
                    } catch (failure: unknown) {
                      setError(describeFailure(failure));
                    } finally {
                      setBusy(false);
                    }
                  })();
                }}
              >
                {t("删除方案")}
              </button>
            </div>
            <div className="settings-form">
              <label>
                {t("规则类型")}
                <select
                  aria-label={t("规则类型")}
                  disabled={busy || connected}
                  value={routeRuleKind}
                  onChange={(event) =>
                    setRouteRuleKind(event.target.value as RouteRuleKind)
                  }
                >
                  {Object.entries(ROUTE_KIND_LABEL).map(([value, label]) => (
                    <option key={value} value={value}>
                      {t(label)}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                {t("规则值")}
                <input
                  aria-label={t("规则值")}
                  disabled={busy || connected}
                  placeholder={
                    routeRuleKind === "network"
                      ? "tcp 或 udp"
                      : routeRuleKind === "processName"
                        ? "chrome"
                        : routeRuleKind === "processPath"
                          ? "/Applications/Safari.app"
                          : undefined
                  }
                  value={routeRuleValue}
                  onChange={(event) => setRouteRuleValue(event.target.value)}
                />
              </label>
              <label>
                {t("出口")}
                <select
                  aria-label={t("规则出口")}
                  disabled={busy || connected}
                  value={routeRuleOutbound}
                  onChange={(event) =>
                    setRouteRuleOutbound(event.target.value as RouteOutbound)
                  }
                >
                  <option value="proxy">{t("代理")}</option>
                  <option value="direct">{t("直连")}</option>
                </select>
              </label>
            </div>
            <div className="actions">
              <button
                type="button"
                disabled={busy || connected}
                onClick={onAddRouteRule}
              >
                {t("添加规则")}
              </button>
            </div>

            {routeDraft.rules.length === 0 ? (
              <p className="hint">{t("尚未添加规则")}</p>
            ) : (
              <table className="node-list" aria-label={t("路由规则列表")}>
                <thead>
                  <tr>
                    <th>{t("顺序")}</th>
                    <th>{t("类型")}</th>
                    <th>{t("值")}</th>
                    <th>{t("出口")}</th>
                    <th>{t("启用")}</th>
                    <th>{t("操作")}</th>
                  </tr>
                </thead>
                <tbody>
                  {routeDraft.rules.map((rule, index) => (
                    <tr key={`${rule.kind}-${rule.value}-${index}`}>
                      <td>{index + 1}</td>
                      <td>{t(ROUTE_KIND_LABEL[rule.kind])}</td>
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
                          {t("上移")}
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
                          {t("下移")}
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
                          {t("删除")}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}

            <div className="settings-form">
              <label>
                {t("默认出口")}
                <select
                  aria-label={t("默认出口")}
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
                  <option value="proxy">{t("代理")}</option>
                  <option value="direct">{t("直连")}</option>
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
            {t("保存路由")}
          </button>
        </div>

      </Dialog>

      <Dialog
        hidden={dialog !== "dns"}
        title="DNS"
        ariaLabel="DNS"
        onClose={() => setDialog(null)}
        wide
      >
        <h2>DNS</h2>

        {dnsDraft === null ? (
          <p className="hint">{t("正在读取 DNS 设置")}</p>
        ) : (
          <div className="settings-form">
            <label>
              {t("模板")}
              <select
                aria-label={t("DNS 模板")}
                disabled={busy || connected}
                value={dnsDraft.template}
                onChange={(event) => {
                  const template = event.target.value as DnsTemplate;
                  const patch =
                    template === "advanced"
                      ? {
                          mode: "doh" as const,
                          server: "cloudflare-dns.com",
                          port: 443,
                          dohPath: "/dns-query",
                          bootstrap: "223.5.5.5",
                          fakeIpEnabled: true,
                        }
                      : {
                          mode: "system" as const,
                          bootstrap: "",
                          fakeIpEnabled: false,
                        };
                  setDnsDraft({ ...dnsDraft, ...patch, template });
                  setDnsDirty(true);
                }}
              >
                <option value="simple">{t("简易")}</option>
                <option value="advanced">{t("高级")}</option>
              </select>
            </label>
            <label>
              {t("模式")}
              <select
                aria-label={t("DNS 模式")}
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
                <option value="system">{t("系统 DNS")}</option>
                <option value="plainUdp">UDP</option>
                <option value="plainTcp">TCP</option>
                <option value="doh">DoH</option>
                <option value="dot">DoT</option>
              </select>
            </label>
            {dnsDraft.mode !== "system" && (
              <>
                <label>
                  {t("Bootstrap")}
                  <input
                    aria-label={t("Bootstrap DNS")}
                    disabled={busy || connected}
                    placeholder="223.5.5.5"
                    value={dnsDraft.bootstrap}
                    onChange={(event) => {
                      setDnsDraft({ ...dnsDraft, bootstrap: event.target.value });
                      setDnsDirty(true);
                    }}
                  />
                </label>
                <label>
                  {t("服务器")}
                  <input
                    aria-label={t("DNS 服务器")}
                    disabled={busy || connected}
                    value={dnsDraft.server}
                    onChange={(event) => {
                      setDnsDraft({ ...dnsDraft, server: event.target.value });
                      setDnsDirty(true);
                    }}
                  />
                </label>
                <label>
                  {t("端口")}
                  <input
                    aria-label={t("DNS 端口")}
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
                {t("DoH 路径")}
                <input
                  aria-label={t("DoH 路径")}
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
              {t("地址策略")}
              <select
                aria-label={t("DNS 地址策略")}
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
                <option value="preferIpv4">{t("优先 IPv4")}</option>
                <option value="preferIpv6">{t("优先 IPv6")}</option>
                <option value="ipv4Only">{t("仅 IPv4")}</option>
                <option value="ipv6Only">{t("仅 IPv6")}</option>
              </select>
            </label>
            <label>
              {t("Hosts")}
              <textarea
                aria-label={t("DNS Hosts")}
                disabled={busy || connected}
                rows={3}
                placeholder={t("每行一条：example.com 1.2.3.4")}
                value={dnsDraft.hosts}
                onChange={(event) => {
                  setDnsDraft({ ...dnsDraft, hosts: event.target.value });
                  setDnsDirty(true);
                }}
              />
            </label>
            <label>
              {t("系统 DNS 域名后缀")}
              <textarea
                aria-label={t("系统 DNS 域名后缀")}
                disabled={busy || connected}
                rows={2}
                placeholder={t("每行一个，例如 lan")}
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
                aria-label={t("启用 IPv6 DNS")}
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
                aria-label={t("启用 FakeIP")}
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
            {t("保存 DNS")}
          </button>
        </div>

      </Dialog>

      <Dialog
        hidden={dialog !== "geo"}
        title={t("更新 Geo 文件")}
        ariaLabel={t("更新 Geo 文件")}
        onClose={() => setDialog(null)}
      >
        <h2>{t("更新 Geo 文件")}</h2>
        <p className="hint">
          {t("下载 Xray 使用的 geoip.dat / geosite.dat。sing-box 在连接时按需拉取远程规则集。")}
        </p>
        {geoStatus === null ? (
          <p className="hint">{t("正在读取 Geo 状态")}</p>
        ) : (
          <div className="settings-form">
            <p className="hint">
              {t("目录")}：{geoStatus.directory}
            </p>
            <p>
              geoip.dat —{" "}
              {geoStatus.geoip.present
                ? `${formatBytes(geoStatus.geoip.bytes)}${
                    geoStatus.geoip.modifiedAt === null
                      ? ""
                      : ` · ${formatClock(geoStatus.geoip.modifiedAt * 1000)}`
                  }`
                : t("未下载")}
            </p>
            <p>
              geosite.dat —{" "}
              {geoStatus.geosite.present
                ? `${formatBytes(geoStatus.geosite.bytes)}${
                    geoStatus.geosite.modifiedAt === null
                      ? ""
                      : ` · ${formatClock(geoStatus.geosite.modifiedAt * 1000)}`
                  }`
                : t("未下载")}
            </p>
            <p className="hint">
              {geoStatus.assetEnvApplied
                ? t("已准备就绪（下次启动 Xray 时生效）")
                : t("下载完成后，下次启动 Xray 时生效")}
            </p>
          </div>
        )}
        <div className="actions">
          <button
            type="button"
            disabled={busy}
            onClick={() => void onUpdateGeo()}
          >
            {t("立即更新")}
          </button>
        </div>
      </Dialog>

      <Dialog
        hidden={dialog !== "settings"}
        title={t("设置")}
        ariaLabel={t("设置")}
        onClose={() => setDialog(null)}
        wide
      >
        <h2>{t("设置")}</h2>

        {settings === null ? (
          <p className="hint">{t("正在读取设置…")}</p>
        ) : (
          <div className="settings-form" aria-label={t("应用设置")}>
            <label className="checkbox-label">
              <input
                aria-label={t("启用 TUN")}
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
              {t("使用 TUN 接管全局流量")}
            </label>
            <p className="hint">
              TUN：{platform ? TUN_LABEL[platform.tunAvailability] : "—"}
            </p>
            <p className="hint">
              {platform ? TUN_NOTICE[platform.tunAvailability] : ""}
              TUN 与系统代理互斥，启用后本次会话不会修改系统代理。
            </p>
            <label className="checkbox-label">
              <input
                aria-label={t("开机启动")}
                type="checkbox"
                checked={settings.launchAtLogin}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({ launchAtLogin: event.target.checked })
                }
              />
              {t("登录系统时自动启动 MgClash")}
            </label>
            <label className="checkbox-label">
              <input
                aria-label={t("启动时自动连接")}
                type="checkbox"
                checked={settings.connectOnLaunch}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({
                    connectOnLaunch: event.target.checked,
                  })
                }
              />
              {t("启动时自动连接上次选中的节点")}
            </label>
            <label className="checkbox-label">
              <input
                aria-label={t("关闭时最小化到托盘")}
                type="checkbox"
                checked={settings.closeToTray}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({ closeToTray: event.target.checked })
                }
              />
              {t("关闭窗口时最小化到托盘，而不是退出")}
            </label>
            <label>
              Core
              <select
                aria-label={t("Core 选择")}
                value={settings.corePreference}
                disabled={busy || connected}
                onChange={(event) =>
                  void onChangeSettings({
                    corePreference: event.target.value as CorePreference,
                  })
                }
              >
                <option value="auto">{t("自动")}</option>
                <option value="sing-box">sing-box</option>
                <option value="xray">Xray</option>
              </select>
            </label>
            <p className="hint">
              {t("自动模式按节点协议和能力矩阵决定。Xray 不支持 Hysteria2 / TUIC，选中后遇到该协议的节点会提示原因。")}
            </p>
            <label>
              {t("默认日志级别")}
              <select
                aria-label={t("默认日志级别")}
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
            <label className="checkbox-label">
              <input
                aria-label={t("允许来自局域网的连接")}
                type="checkbox"
                checked={settings.allowLan}
                disabled={busy || connected}
                onChange={(event) =>
                  void onChangeSettings({ allowLan: event.target.checked })
                }
              />
              {t("允许来自局域网的连接")}
            </label>
            <p className="hint">
              {t("开启后本地 SOCKS/HTTP 监听 0.0.0.0，局域网设备可使用本机代理；Clash API 仍仅本机可访问。下次连接生效。")}
            </p>
            <label className="checkbox-label">
              <input
                aria-label={t("启用入站 UDP")}
                type="checkbox"
                checked={settings.inboundUdpEnabled}
                disabled={busy || connected}
                onChange={(event) =>
                  void onChangeSettings({
                    inboundUdpEnabled: event.target.checked,
                  })
                }
              />
              {t("启用 SOCKS 入站 UDP（Xray，下次连接生效）")}
            </label>
            <label className="checkbox-label">
              <input
                aria-label={t("启用 Mux")}
                type="checkbox"
                checked={settings.muxEnabled}
                disabled={busy || connected}
                onChange={(event) =>
                  void onChangeSettings({ muxEnabled: event.target.checked })
                }
              />
              {t("启用 Mux 多路复用（下次连接生效）")}
            </label>
            <p className="hint">
              {t("sing-box 使用 h2mux；Xray 使用 mux。含 Vision flow 的 VLESS 与 Hysteria2 / TUIC 会自动跳过。")}
            </p>
            <label className="checkbox-label">
              <input
                aria-label={t("启用 Fragment")}
                type="checkbox"
                checked={settings.fragmentEnabled}
                disabled={busy || connected}
                onChange={(event) =>
                  void onChangeSettings({
                    fragmentEnabled: event.target.checked,
                  })
                }
              />
              {t("启用 Fragment 反检测（下次连接生效）")}
            </label>
            <p className="hint">
              {t("将 TLS ClientHello 拆分发送以规避基于明文特征的检测；sing-box 使用 TLS fragment/record_fragment，Xray 使用 freedom fragment 出站。仅对含 TLS 握手的节点生效。")}
            </p>
            <label className="checkbox-label">
              <input
                aria-label={t("启用 Final Fragment")}
                type="checkbox"
                checked={settings.finalFragmentEnabled}
                disabled={busy || connected}
                onChange={(event) =>
                  void onChangeSettings({
                    finalFragmentEnabled: event.target.checked,
                  })
                }
              />
              {t("启用 Final Fragment 尾部分片（下次连接生效）")}
            </label>
            <p className="hint">
              {t("在最终落地阶段拆分 TLS 记录；sing-box 使用 route-options tls_record_fragment，Xray 使用 freedom finalmask 包装代理出站。")}
            </p>
            <label className="checkbox-label">
              <input
                aria-label={t("启用 UDP Noise")}
                type="checkbox"
                checked={settings.udpNoiseEnabled}
                disabled={busy || connected}
                onChange={(event) =>
                  void onChangeSettings({
                    udpNoiseEnabled: event.target.checked,
                  })
                }
              />
              {t("启用 UDP Noise 反检测（下次连接生效）")}
            </label>
            <p className="hint">
              {t("在真实 UDP 数据前发送随机噪声包以规避嗅探；仅 Xray 生效（freedom noises），sing-box 无对应能力。默认 length 10-20、delay 10-16。")}
            </p>
            <label className="checkbox-label">
              <input
                aria-label={t("测速后自动选择最低延迟")}
                type="checkbox"
                checked={settings.autoSelectLowestLatency}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({
                    autoSelectLowestLatency: event.target.checked,
                  })
                }
              />
              {t("全部测速后自动选择延迟最低的节点")}
            </label>
            <label>
              {t("URL 测试地址")}
              <input
                aria-label={t("设置中的 URL 测试地址")}
                value={settings.urlTestAddress}
                disabled={busy}
                onChange={(event) => {
                  const urlTestAddress = event.target.value;
                  setUrlTestAddress(urlTestAddress);
                  setSettings({ ...settings, urlTestAddress });
                }}
                onBlur={() => {
                  void onChangeSettings({
                    urlTestAddress:
                      settings.urlTestAddress.trim() || DEFAULT_URL_TEST_ADDRESS,
                  });
                }}
              />
            </label>
            <label>
              {t("下载测速地址")}
              <input
                aria-label={t("下载测速地址")}
                value={settings.speedTestUrl}
                disabled={busy}
                onChange={(event) =>
                  setSettings({ ...settings, speedTestUrl: event.target.value })
                }
                onBlur={() => {
                  void onChangeSettings({
                    speedTestUrl:
                      settings.speedTestUrl.trim() || DEFAULT_SPEED_TEST_URL,
                  });
                }}
              />
            </label>
            <label className="checkbox-label">
              <input
                aria-label={t("默认允许不安全证书")}
                type="checkbox"
                checked={settings.defAllowInsecure}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({
                    defAllowInsecure: event.target.checked,
                  })
                }
              />
              {t("新建节点默认允许不安全证书")}
            </label>
            <label>
              {t("默认 TLS 指纹")}
              <input
                aria-label={t("默认 TLS 指纹")}
                value={settings.defFingerprint}
                disabled={busy}
                placeholder="chrome"
                onChange={(event) =>
                  setSettings({
                    ...settings,
                    defFingerprint: event.target.value,
                  })
                }
                onBlur={() => {
                  void onChangeSettings({
                    defFingerprint: settings.defFingerprint.trim(),
                  });
                }}
              />
            </label>
            <p className="hint">
              {t("用于手动创建节点时的 TLS 默认值，不影响已有节点。")}
            </p>
            <label>
              {t("热键：连接/断开")}
              <input
                aria-label={t("热键：连接/断开")}
                value={settings.hotkeyConnect}
                disabled={busy}
                placeholder="Ctrl+Enter"
                onChange={(event) =>
                  setSettings({
                    ...settings,
                    hotkeyConnect: event.target.value,
                  })
                }
                onBlur={() => {
                  void onChangeSettings({
                    hotkeyConnect: settings.hotkeyConnect.trim(),
                  });
                }}
              />
            </label>
            <label>
              {t("热键：上一节点")}
              <input
                aria-label={t("热键：上一节点")}
                value={settings.hotkeyPrevious}
                disabled={busy}
                placeholder="Ctrl+["
                onChange={(event) =>
                  setSettings({
                    ...settings,
                    hotkeyPrevious: event.target.value,
                  })
                }
                onBlur={() => {
                  void onChangeSettings({
                    hotkeyPrevious: settings.hotkeyPrevious.trim(),
                  });
                }}
              />
            </label>
            <label>
              {t("热键：下一节点")}
              <input
                aria-label={t("热键：下一节点")}
                value={settings.hotkeyNext}
                disabled={busy}
                placeholder="Ctrl+]"
                onChange={(event) =>
                  setSettings({
                    ...settings,
                    hotkeyNext: event.target.value,
                  })
                }
                onBlur={() => {
                  void onChangeSettings({
                    hotkeyNext: settings.hotkeyNext.trim(),
                  });
                }}
              />
            </label>
            <p className="hint">
              {t("热键在系统全局生效；窗口未聚焦时也可使用。留空表示禁用。")}
            </p>
            <p className="hint">
              {t("窗口内生效；留空表示禁用。输入框获得焦点时不触发。")}
            </p>
            <label>
              {t("SOCKS 端口")}
              <input
                aria-label={t("SOCKS 端口")}
                type="number"
                min="1"
                max="65535"
                disabled={busy || connected}
                value={settings.socksPort}
                onChange={(event) =>
                  void onChangeSettings({
                    socksPort: Number(event.target.value),
                  })
                }
              />
            </label>
            <label>
              {t("HTTP 端口")}
              <input
                aria-label={t("HTTP 端口")}
                type="number"
                min="1"
                max="65535"
                disabled={busy || connected}
                value={settings.httpPort}
                onChange={(event) =>
                  void onChangeSettings({
                    httpPort: Number(event.target.value),
                  })
                }
              />
            </label>
            <label>
              {t("Clash API 端口")}
              <input
                aria-label={t("Clash API 端口")}
                type="number"
                min="1"
                max="65535"
                disabled={busy || connected}
                value={settings.clashApiPort}
                onChange={(event) =>
                  void onChangeSettings({
                    clashApiPort: Number(event.target.value),
                  })
                }
              />
            </label>
            <p className="hint" aria-label={t("本地代理端口")}>
              {t("本地代理端口在下次连接时生效；SOCKS、HTTP 与 Clash API 不能相同。")}
            </p>
            <p className="hint">
              {t("PAC 模式会启动本地 PAC 服务并写入系统代理（全局脚本，不等价于规则模式）。")}
            </p>
            <p className="hint">
              {t("构建目标")}：{platform ? platform.artifactIdentifier : platformError}
            </p>
          </div>
        )}

      </Dialog>

      <Dialog
        hidden={dialog !== "about"}
        title={t("关于")}
        ariaLabel={t("关于")}
        onClose={() => setDialog(null)}
      >
        <p>MgClash</p>
        <p>{platform?.artifactIdentifier}</p>
        <p>{t("界面以 v2rayN Avalonia 为模板重新实现，未使用其 GPL 源码。")}</p>
        <p>
          {t("关闭窗口时最小化到托盘")}: {settings?.closeToTray ? t("已启用") : t("未启用")}
        </p>
        <p className="hint">
          {t("托盘菜单支持打开窗口、连接/断开、切换路由模式与选择节点。")}
        </p>
      </Dialog>

    </main>
  );
}
