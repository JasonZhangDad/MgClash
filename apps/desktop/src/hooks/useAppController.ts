import { DEFAULT_LOCALE, translate, type Locale } from "../i18n";
import { useCallback, useEffect, useRef, useState } from "react";

import {
  buildManualNodeDraft,
  blankManualNodeForm,
  emptyManualNodeForm,
  formFromManualNodeDraft,
  type ManualNodeForm,
} from "../manualNode";
import { loadPlatformSummary, type PlatformSummary } from "../platform";
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
  closeConnection,
  closeConnections,
  loadConnections,
  loadRuleSets,
  updateRuleSet,
  updateRuleSets,
  loadCoreReadiness,
  importNode,
  importNodes,
  loadAppSettings,
  previewCoreConfig,
  loadLogs,
  loadNodeDraft,
  type ManualNodeDraft,
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
  requestAppExit,
  selectNode,
  switchNode,
  setDnsSettings,
  setNodeEnabled,
  setNodeGroup,
  setNodeFront,
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
  type RuleProviderFormat,
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
  type ConnectionSnapshot,
  type RuleSetCacheEntry,
  type CoreReadiness,
} from "../session";
import {
  createSubscription,
  deleteSubscription,
  loadSubscriptions,
  refreshAllSubscriptions,
  refreshSubscription,
  updateSubscription,
  type SubscriptionSummary,
} from "../subscriptions";
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

  ROUTE_KIND_LABEL,
  ROUTE_OUTBOUND_LABEL,
  clampColumnWidth,
  loadColumnWidths,
  nextNodeSort,
  saveColumnWidths,
  sortNodes,
  type NodeSort,
  type NodeSortColumn,
  ruleDraftFromConnection,
  describeRuleSetCache,
  groupTraffic,
  processTraffic,
  runtimeOrderedRoute,
  savedFontSize,
  savedLayout,
  savedTheme,
  savedUrlTestAddress,
  sortNodeIdsByLatency,
  lowestLatencyNodeId,
  matchesHotkey,
  FONT_SIZE_KEY,
  THEME_KEY,
  TRAFFIC_REFRESH_INTERVAL_MS,
  CONNECTIONS_REFRESH_INTERVAL_MS,
  TUN_NOTICE,
  REFRESH_INTERVAL_MS,
  TUN_LABEL,
  UNSIGNED_NOTICE_KEY,
  URL_TEST_ADDRESS_KEY,
  type AppPage,
  type DialogId,
  type FontSize,
  type InspectTab,
  type MainLayout,
  type MainTab,
  type NodeMenuPosition,
  type NodesTab,
  type SettingsSection,
  type ThemeMode,
  type TrafficSample,
} from "../appHelpers";
import {
  clearGlobalHotkeys,
  syncGlobalHotkeys,
} from "../globalHotkeys";
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
  daily: [],
  downloadBytesPerSecond: 0,
  monthBytes: 0,
  todayBytes: 0,
  totalBytes: 0,
  uploadBytesPerSecond: 0,
};

export function useAppController() {
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
  const [ruleSets, setRuleSets] = useState<RuleSetCacheEntry[]>([]);
  const [providerName, setProviderName] = useState("");
  const [providerUrl, setProviderUrl] = useState("");
  const [providerFormat, setProviderFormat] =
    useState<RuleProviderFormat>("binary");
  const [providerOutbound, setProviderOutbound] =
    useState<RouteOutbound>("direct");
  const [routeDirty, setRouteDirty] = useState(false);
  const [routeRuleKind, setRouteRuleKind] = useState<RouteRuleKind>("domainSuffix");
  const [routeRuleValue, setRouteRuleValue] = useState("");
  /// The generated document, read on demand and editable into an override.
  const [coreConfig, setCoreConfig] = useState("");
  /// Edited as text and saved on demand: a template is not valid on most
  /// keystrokes, so saving it on change would reject what the user is still
  /// typing. Seeded from the saved settings, so the panel opens showing what
  /// is in force rather than an empty box that reads as "no template".
  const [configTemplate, setConfigTemplate] = useState("");
  const [routeRuleOutbound, setRouteRuleOutbound] =
    useState<RouteOutbound>("proxy");
  const [nodes, setNodes] = useState<NodeSummary[]>([]);
  const [nodeGroups, setNodeGroups] = useState<NodeGroupSummary[]>([]);
  const [nodeSort, setNodeSort] = useState<NodeSort | null>(null);
  const [columnWidths, setColumnWidths] = useState(loadColumnWidths);
  const [nodeGroupFilter, setNodeGroupFilter] = useState("all");
  const [groupingNodeId, setGroupingNodeId] = useState<string | null>(null);
  const [groupingNodeIds, setGroupingNodeIds] = useState<string[]>([]);
  const [frontingNodeId, setFrontingNodeId] = useState<string | null>(null);
  const [frontChoice, setFrontChoice] = useState("");
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
  const [settingsTab, setSettingsTab] = useState<
    "core" | "ui" | "tun" | "coreType"
  >("core");
  const [theme, setTheme] = useState<ThemeMode>(() => savedTheme());
  const [fontSize, setFontSize] = useState<FontSize>(() => savedFontSize());
  const [layout, setLayout] = useState<MainLayout>(() => savedLayout());
  const [msgVisible, setMsgVisible] = useState(true);
  const [mainTab, setMainTab] = useState<MainTab>("profiles");
  const [sideTab, setSideTab] = useState<Exclude<MainTab, "profiles">>("msg");
  const [connections, setConnections] = useState<ConnectionSnapshot | null>(
    null,
  );
  const [connectionQuery, setConnectionQuery] = useState("");
  const [proxyGroupId, setProxyGroupId] = useState<string | null>(null);
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
  const [coreReadiness, setCoreReadiness] = useState<CoreReadiness | null>(
    null,
  );
  const [page, setPage] = useState<AppPage>("overview");
  const [nodesTab, setNodesTab] = useState<NodesTab>("all");
  const [settingsSection, setSettingsSection] =
    useState<SettingsSection>("general");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [inspectTab, setInspectTab] = useState<InspectTab>("general");
  const [inspectDraft, setInspectDraft] = useState<ManualNodeDraft | null>(null);
  const [trafficHistory, setTrafficHistory] = useState<TrafficSample[]>([]);
  const [selectedConnectionId, setSelectedConnectionId] = useState<
    string | null
  >(null);
  const [connectionProcess, setConnectionProcess] = useState("");
  const [connectionNetwork, setConnectionNetwork] = useState("");
  const [connectionRule, setConnectionRule] = useState("");
  const [connectionChain, setConnectionChain] = useState("");
  const [logPaused, setLogPaused] = useState(false);
  const [logAutoScroll, setLogAutoScroll] = useState(true);
  const [logFilter, setLogFilter] = useState("");
  const [nodeView, setNodeView] = useState<"list" | "cards">("list");
  const [portsOpen, setPortsOpen] = useState(false);
  const [proxyPopoverOpen, setProxyPopoverOpen] = useState(false);
  const [refreshingSubscriptions, setRefreshingSubscriptions] = useState(false);

  const nodeTestInProgress =
    testingSpeed ||
    Object.values(nodeTests).some((result) => result.status === "testing");

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.style.fontSize = `${fontSize}px`;
    try {
      localStorage.setItem(THEME_KEY, theme);
      localStorage.setItem(FONT_SIZE_KEY, String(fontSize));
    } catch {
      // The theme still applies to this launch even if it cannot be stored.
    }
  }, [theme, fontSize]);

  useEffect(() => {
    try {
      localStorage.setItem(LAYOUT_KEY, layout);
    } catch {
      // The layout still applies to this launch even if it cannot be stored.
    }
  }, [layout]);

  useEffect(() => {
    // Follows the saved value, which changes on load and after a save; an edit
    // in progress is never overwritten, because nothing else moves it.
    setConfigTemplate(settings?.configTemplate ?? "");
  }, [settings?.configTemplate]);

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
      if (!logPaused) {
        loadLogs(logLevel, logSource === "all" ? null : logSource).then(
          setLogs,
          (failure: unknown) => {
            console.warn("log refresh failed", failure);
          },
        );
      }
      loadSessionStatus().then(setStatus, (failure: unknown) => {
        // A background refresh must not replace an error the user is reading.
        console.warn("session status refresh failed", failure);
      });
      loadSubscriptions().then(setSubscriptions, (failure: unknown) => {
        console.warn("subscription refresh failed", failure);
      });
    }, REFRESH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [busy, logLevel, logPaused, logSource]);

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
          setTrafficHistory((current) =>
            [
              ...current,
              {
                down: sample.downloadBytesPerSecond,
                up: sample.uploadBytesPerSecond,
              },
            ].slice(-60),
          );
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

  // Only polled while a page that reads live connections is open.
  useEffect(() => {
    if (
      (page !== "connections" && page !== "traffic") ||
      status?.connected !== true ||
      status.core === "xray"
    ) {
      if (page !== "connections" && page !== "traffic") {
        setConnections(null);
      }
      return undefined;
    }
    let active = true;
    const refresh = () => {
      loadConnections().then(
        (snapshot) => {
          if (active) {
            setConnections(snapshot);
          }
        },
        (failure: unknown) => {
          console.warn("connection refresh failed", failure);
        },
      );
    };
    refresh();
    const timer = setInterval(refresh, CONNECTIONS_REFRESH_INTERVAL_MS);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [page, status?.connected, status?.core]);

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
      return false;
    }
    const server = dnsDraft.server.trim();
    if (dnsDraft.mode !== "system" && server === "") {
      setError("请填写 DNS 服务器");
      return false;
    }
    if (
      dnsDraft.mode !== "system" &&
      (!Number.isInteger(dnsDraft.port) ||
        dnsDraft.port < 1 ||
        dnsDraft.port > 65_535)
    ) {
      setError("DNS 端口必须是 1 到 65535 的整数");
      return false;
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
      return true;
    } catch (failure: unknown) {
      setError(describeFailure(failure));
      return false;
    } finally {
      setBusy(false);
    }
  }, [dnsDraft]);

  const onUpdateRuleSet = useCallback(
    async (name: string) => {
      setBusy(true);
      setError(null);
      try {
        setRuleSets(await updateRuleSet(name));
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const onUpdateRuleSets = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setRuleSets(await updateRuleSets());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onAddRuleProvider = useCallback(() => {
    const name = providerName.trim();
    const url = providerUrl.trim();
    if (routeDraft === null || name === "" || url === "") {
      return;
    }
    setRouteDraft({
      ...routeDraft,
      providers: [
        ...routeDraft.providers,
        {
          enabled: true,
          format: providerFormat,
          name,
          outbound: providerOutbound,
          url,
        },
      ],
    });
    setProviderName("");
    setProviderUrl("");
    setRouteDirty(true);
  }, [providerFormat, providerName, providerOutbound, providerUrl, routeDraft]);

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

  const onImportClipboard = useCallback(async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text.trim() === "") {
        setError("剪贴板为空");
        return;
      }
      await runBulkImport(text);
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    }
  }, [runBulkImport]);

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

  const refreshCoreReadiness = useCallback(async () => {
    try {
      setCoreReadiness(await loadCoreReadiness());
    } catch (failure: unknown) {
      console.warn("core readiness check failed", failure);
    }
  }, []);

  // Checked once at startup, the way v2rayN notices it has no Core to run.
  useEffect(() => {
    void refreshCoreReadiness();
  }, [refreshCoreReadiness]);

  const onDownloadCore = useCallback(
    async (core: "sing-box" | "xray") => {
      setBusy(true);
      setError(null);
      setExportedTo(null);
      try {
        await downloadCoreUpdate(core);
        setCoreUpdate(await checkCoreUpdate());
        setExportedTo(t("Core 已下载并安装，下次连接时生效"));
        await refreshCoreReadiness();
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [refreshCoreReadiness, t],
  );

  const onOpenRouting = useCallback(async () => {
    setPage("routing");
    setError(null);
    try {
      setRuleSets(await loadRuleSets());
    } catch (failure: unknown) {
      console.warn("rule set status failed", failure);
    }
  }, []);

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
    setGroupingNodeIds([]);
    setNodeGroupName("");
    setNodeGroupStrategy("select");
  }, []);

  const onSaveNodeFront = useCallback(async () => {
    if (frontingNodeId === null) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setNodes(await setNodeFront(frontingNodeId, frontChoice || null));
      setFrontingNodeId(null);
      setFrontChoice("");
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [frontChoice, frontingNodeId]);

  const onGroupNode = useCallback(
    (candidate: NodeSummary, ids?: string[]) => {
      const batch = ids !== undefined && ids.length > 0 ? ids : [candidate.id];
      setGroupingNodeId(candidate.id);
      setGroupingNodeIds(batch);
      const group = nodeGroups.find((item) => item.id === candidate.groupId);
      setNodeGroupName(group?.name ?? "");
      setNodeGroupStrategy(group?.strategy ?? "select");
    },
    [nodeGroups],
  );

  const onSaveNodeGroup = useCallback(async () => {
    const ids =
      groupingNodeIds.length > 0
        ? groupingNodeIds
        : groupingNodeId === null
          ? []
          : [groupingNodeId];
    if (ids.length === 0) {
      return;
    }
    const groupName = nodeGroupName.trim();
    setBusy(true);
    setError(null);
    try {
      let next = nodes;
      for (const id of ids) {
        next = await setNodeGroup(id, groupName || null);
      }
      setNodes(next);
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
  }, [
    groupingNodeId,
    groupingNodeIds,
    nodeGroupName,
    nodeGroupStrategy,
    nodes,
    resetNodeGroupForm,
  ]);

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

  const onActivateNode = useCallback(
    async (id: string) => {
      setBusy(true);
      setError(null);
      try {
        if (status?.connected === true) {
          setStatus(await switchNode(id));
        } else {
          if (status?.node?.id !== id) {
            await selectNode(id);
          }
          setStatus(await connectSession());
        }
        setNodes(await loadNodes());
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [status?.connected, status?.node?.id],
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
    setRefreshingSubscriptions(true);
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
      setRefreshingSubscriptions(false);
    }
  }, [syncNodes]);

  const onRefreshAllSubscriptions = useCallback(async () => {
    setBusy(true);
    setRefreshingSubscriptions(true);
    setError(null);
    try {
      setSubscriptions(await refreshAllSubscriptions());
      await syncNodes();
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
      setRefreshingSubscriptions(false);
    }
  }, [syncNodes]);

  const onToggleSubscriptionEnabled = useCallback(
    async (item: SubscriptionSummary, enabled: boolean) => {
      setBusy(true);
      setError(null);
      try {
        const saved = await updateSubscription({
          autoUpdate: item.autoUpdate,
          enabled,
          excludeKeywords: item.excludeKeywords,
          id: item.id,
          includeKeywords: item.includeKeywords,
          name: item.name,
          updateIntervalMinutes: item.updateIntervalMinutes,
          url: null,
          userAgent: item.userAgent,
          subconverterUrl: item.subconverterUrl,
        });
        setSubscriptions((current) =>
          current.map((entry) => (entry.id === item.id ? saved : entry)),
        );
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

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
  const onSortColumn = (column: NodeSortColumn) => {
    setNodeSort((current) => nextNodeSort(current, column));
  };
  const onResizeColumn = useCallback((label: string, width: number) => {
    setColumnWidths((current) => {
      const next = { ...current, [label]: width };
      saveColumnWidths(next);
      return next;
    });
  }, []);
  const visibleNodes = sortNodes(
    nodes.filter((candidate) => {
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
    }),
    nodeSort,
  );
  const inspected =
    inspectedId === null
      ? null
      : (nodes.find((candidate) => candidate.id === inspectedId) ?? null);

  useEffect(() => {
    if (inspectedId === null) {
      setInspectDraft(null);
      return undefined;
    }
    let cancelled = false;
    void Promise.resolve(loadNodeDraft(inspectedId)).then(
      (draft) => {
        if (!cancelled) {
          setInspectDraft(draft ?? null);
        }
      },
      () => {
        if (!cancelled) {
          setInspectDraft(null);
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [inspectedId]);

  useEffect(() => {
    if (page === "routing") {
      void loadRuleSets().then(setRuleSets, (failure: unknown) => {
        console.warn("rule set status failed", failure);
      });
    }
  }, [page]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const meta = event.metaKey || event.ctrlKey;
      if (meta && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((current) => !current);
        return;
      }
      // Handoff: Escape closes the topmost overlay, one layer per press, so a
      // dialog opened over the inspector does not take the inspector with it.
      if (event.key === "Escape") {
        setPortsOpen(false);
        setProxyPopoverOpen(false);
        if (paletteOpen) {
          setPaletteOpen(false);
          return;
        }
        if (dialog !== null) {
          setDialog(null);
          if (dialog === "create") {
            resetNodeForm();
          }
          return;
        }
        if (inspectedId !== null) {
          setInspectedId(null);
        }
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [dialog, inspectedId, paletteOpen, resetNodeForm]);

  const onToggleConnect = useCallback(() => {
    if (busy) {
      return;
    }
    if (connected) {
      void run(disconnectSession);
      return;
    }
    if (node !== null && systemProxyStartup === "clean") {
      void run(connectSession);
    }
  }, [busy, connected, node, run, systemProxyStartup]);

  const onRestartCore = useCallback(() => {
    if (busy) {
      return;
    }
    if (connected) {
      void run(async () => {
        await disconnectSession();
        return connectSession();
      });
      return;
    }
    void onReload();
  }, [busy, connected, onReload, run]);

  const goTo = useCallback((next: AppPage) => {
    setPage(next);
    setPaletteOpen(false);
    if (next === "proxies") {
      setMainTab("proxies");
      setSideTab("proxies");
    } else if (next === "connections") {
      setMainTab("connections");
      setSideTab("connections");
    } else if (next === "logs") {
      setMainTab("msg");
      setSideTab("msg");
      setMsgVisible(true);
    } else if (next === "nodes" || next === "overview") {
      setMainTab("profiles");
    }
    if (next === "routing") {
      void loadRuleSets().then(setRuleSets, (failure: unknown) => {
        console.warn("rule set status failed", failure);
      });
    }
  }, []);

  const openSettings = useCallback((section: SettingsSection = "general") => {
    setSettingsSection(section);
    setPage("settings");
    if (section === "core" || section === "network") {
      setSettingsTab("core");
    } else if (section === "tun") {
      setSettingsTab("tun");
    } else if (section === "appearance" || section === "language") {
      setSettingsTab("ui");
    }
  }, []);

  return {
    bulkReport,
    bulkText,
    busy,
    checkedNodes,
    columnWidths,
    configTemplate,
    connected,
    connectionChain,
    connectionNetwork,
    connectionProcess,
    connectionQuery,
    connectionRule,
    connections,
    coreConfig,
    coreReadiness,
    coreUpdate,
    createForm,
    dialog,
    dismissNotice,
    dnsDirty,
    dnsDraft,
    editingNodeId,
    editingSubscriptionId,
    error,
    exportedTo,
    fontSize,
    formatCoreVersionLine,
    formatInstalledCoreLine,
    frontChoice,
    frontingNodeId,
    geoStatus,
    goTo,
    groupingNodeId,
    inspectDraft,
    inspectTab,
    inspected,
    inspectedId,
    layout,
    locale,
    logAutoScroll,
    logFilter,
    logLevel,
    logPaused,
    logSource,
    logs,
    node,
    nodeGroupFilter,
    nodeGroupName,
    nodeGroupNames,
    nodeGroupStrategy,
    nodeGroups,
    nodeMenu,
    nodeQuery,
    nodeSort,
    nodeSpeeds,
    nodeTestInProgress,
    nodeTests,
    nodeTraffic,
    nodeView,
    nodes,
    nodesTab,
    noticeDismissed,
    onActivateNode,
    onAddRouteRule,
    onAddRuleProvider,
    onCancelNodeTests,
    onChangeSettings,
    onCheckCoreUpdate,
    onCheckUpdate,
    onClearLogs,
    onClearTraffic,
    onCloneNode,
    onCreateNode,
    onDeleteNode,
    onDeleteNodes,
    onDeleteSubscription,
    onDownloadCore,
    onEditNode,
    onEditSubscription,
    onExport,
    onExportNodeLink,
    onExportNodeLinks,
    onExportPreferences,
    onExportProfile,
    onGroupNode,
    onImport,
    onImportClipboard,
    onImportFile,
    onImportPreferences,
    onImportProfile,
    onImportQrCode,
    onMoveNode,
    onMoveRouteRule,
    onOpenGeo,
    onOpenRouting,
    onRefreshAllSubscriptions,
    onRefreshSubscription,
    onReload,
    onRemoveDuplicateNodes,
    onResizeColumn,
    onRestartCore,
    onSaveDns,
    onSaveNodeFront,
    onSaveNodeGroup,
    onSaveRoute,
    onSaveSubscription,
    onSelectLowestLatency,
    onShowNodeQrCode,
    onSortColumn,
    onSortNodesByLatency,
    onSpeedTest,
    onStepNode,
    onTestAllNodes,
    onTestNode,
    onTestNodes,
    onTestUrl,
    onToggleConnect,
    onToggleSubscriptionEnabled,
    onUpdateGeo,
    onUpdateRuleSet,
    onUpdateRuleSets,
    openCreateForm,
    openNodeMenu,
    openSettings,
    page,
    paletteOpen,
    platform,
    platformError,
    portsOpen,
    providerFormat,
    providerName,
    providerOutbound,
    providerUrl,
    proxyGroupId,
    proxyPopoverOpen,
    qrCode,
    refreshingSubscriptions,
    refreshLogs,
    resetNodeForm,
    resetNodeGroupForm,
    resetSubscriptionForm,
    resolveSystemProxyStartup,
    routeDirty,
    routeDraft,
    routeRuleKind,
    routeRuleOutbound,
    routeRuleValue,
    ruleSets,
    run,
    runBulkImport,
    selectedConnectionId,
    setBulkReport,
    setBulkText,
    setBusy,
    setCheckedNodes,
    setColumnWidths,
    setConfigTemplate,
    setConnectionChain,
    setConnectionNetwork,
    setConnectionProcess,
    setConnectionQuery,
    setConnectionRule,
    setConnections,
    setCoreConfig,
    setCoreUpdate,
    setCreateForm,
    setDialog,
    setDnsDirty,
    setDnsDraft,
    setEditingNodeId,
    setError,
    setExportedTo,
    setFontSize,
    setFrontChoice,
    setFrontingNodeId,
    setGeoStatus,
    setGroupingNodeId,
    setInspectTab,
    setInspectedId,
    setLayout,
    setLocale,
    setLogAutoScroll,
    setLogFilter,
    setLogLevel,
    setLogPaused,
    setLogSource,
    setLogs,
    setNodeGroupFilter,
    setNodeGroupName,
    setNodeGroupStrategy,
    setNodeGroups,
    setNodeMenu,
    setNodeQuery,
    setNodeSort,
    setNodeView,
    setNodes,
    setNodesTab,
    setPage,
    setPaletteOpen,
    setPortsOpen,
    setProviderFormat,
    setProviderName,
    setProviderOutbound,
    setProviderUrl,
    setProxyGroupId,
    setProxyPopoverOpen,
    setQrCode,
    setRefreshingSubscriptions,
    setRouteDirty,
    setRouteDraft,
    setRouteRuleKind,
    setRouteRuleOutbound,
    setRouteRuleValue,
    setRuleSets,
    setSelectedConnectionId,
    setSettings,
    setSettingsSection,
    setSettingsTab,
    setStatus,
    setSubscriptionAutoUpdate,
    setSubscriptionEnabled,
    setSubscriptionExclude,
    setSubscriptionInclude,
    setSubscriptionInterval,
    setSubscriptionName,
    setSubscriptionSubconverter,
    setSubscriptionUrl,
    setSubscriptionUserAgent,
    setTheme,
    setUpdate,
    setUri,
    setUrlTestAddress,
    settings,
    settingsSection,
    settingsTab,
    status,
    subscriptionAutoUpdate,
    subscriptionEnabled,
    subscriptionExclude,
    subscriptionInclude,
    subscriptionInterval,
    subscriptionName,
    subscriptionSubconverter,
    subscriptionUrl,
    subscriptionUserAgent,
    subscriptions,
    systemProxyStartup,
    t,
    testingAllNodes,
    testingSpeed,
    theme,
    toggleCheckedNode,
    traffic,
    trafficHistory,
    update,
    updateCreateForm,
    uri,
    urlTestAddress,
    visibleNodes,
  };
}

export type AppModel = ReturnType<typeof useAppController>;
