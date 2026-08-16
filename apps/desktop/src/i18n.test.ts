import { describe, expect, it } from "vitest";

// Vite inlines the files at build time, so the test needs no filesystem types
// and reads exactly what the bundler sees. The v2rayN-style shell splits what
// used to be one file across App.tsx and its components, so every file that
// can call `t(...)` has to be scanned, not just App.tsx.
import appSource from "./App.tsx?raw";
import appHelpersSource from "./appHelpers.ts?raw";
import appControllerSource from "./hooks/useAppController.ts?raw";
import cameraScannerSource from "./components/CameraScanner.tsx?raw";
import commandPaletteSource from "./components/CommandPalette.tsx?raw";
import connectionsViewSource from "./components/ConnectionsView.tsx?raw";
import nodeContextMenuSource from "./components/NodeContextMenu.tsx?raw";
import proxiesViewSource from "./components/ProxiesView.tsx?raw";
import dialogSource from "./components/Dialog.tsx?raw";
import menuBarSource from "./components/MenuBar.tsx?raw";
import msgViewSource from "./components/MsgView.tsx?raw";
import appStatusBarSource from "./components/layout/AppStatusBar.tsx?raw";
import sidebarSource from "./components/layout/Sidebar.tsx?raw";
import topBarSource from "./components/layout/TopBar.tsx?raw";
import nodeInspectorSource from "./components/nodes/NodeInspector.tsx?raw";
import nodesPageSource from "./components/nodes/NodesPage.tsx?raw";
import appDialogsSource from "./components/overlays/AppDialogs.tsx?raw";
import overviewSource from "./pages/OverviewPage.tsx?raw";
import trafficSource from "./pages/TrafficPage.tsx?raw";
import connectionsPageSource from "./pages/ConnectionsPage.tsx?raw";
import logsPageSource from "./pages/LogsPage.tsx?raw";
import proxiesPageSource from "./pages/ProxiesPage.tsx?raw";
import routingPageSource from "./pages/RoutingPage.tsx?raw";
import dnsPageSource from "./pages/DnsPage.tsx?raw";
import settingsPageSource from "./pages/SettingsPage.tsx?raw";

import { ROUTE_KIND_LABEL } from "./appHelpers";
import {
  DEFAULT_LOCALE,
  LOCALES,
  translate,
  translatedKeys,
  untranslated,
  type Locale,
} from "./i18n";

/**
 * Every string the components actually ask for, read out of the source.
 *
 * Taken from `App.tsx` rather than from a hand-kept list, because a hand-kept
 * list is what drifts: a new label added without a translation has to fail here,
 * not appear as Chinese in a Russian window.
 */
function usedKeys(): string[] {
  const source = [
    appSource,
    appControllerSource,
    appHelpersSource,
    cameraScannerSource,
    commandPaletteSource,
    connectionsViewSource,
    nodeContextMenuSource,
    proxiesViewSource,
    menuBarSource,
    appStatusBarSource,
    sidebarSource,
    topBarSource,
    nodeInspectorSource,
    nodesPageSource,
    appDialogsSource,
    overviewSource,
    trafficSource,
    connectionsPageSource,
    logsPageSource,
    proxiesPageSource,
    routingPageSource,
    dnsPageSource,
    settingsPageSource,
    msgViewSource,
    dialogSource,
  ].join("\n");
  return [
    ...[...source.matchAll(/\bt\("((?:[^"\\]|\\.)*)"\)/g)].map(
      (match) => match[1],
    ),
    // Reached as `t(label)` with the label held in a record, which the regex
    // above cannot see. The other entries happen to appear literally elsewhere
    // too; relying on that coincidence is how one of them ends up untranslated.
    ...Object.values(ROUTE_KIND_LABEL),
    "通用",
    "网络",
    "内核",
    "外观",
    "数据与备份",
    "高级",
    "路由",
    // Column titles passed to SortableHeader as `label`, then translated
    // there as `t(label)` — the regex above cannot see the string.
    "传输协议",
    "订阅分组",
    "别名",
    "今日上传",
    "今日下载",
    "总上传",
    "总下载",
    "优秀",
    "一般",
    "较差",
    "添加 [VMess]",
    "添加 [VLESS]",
    "添加 [Shadowsocks]",
    "添加 [Trojan]",
    "添加 [Hysteria2]",
    "添加 [WireGuard]",
    "添加 [SOCKS]",
    "添加 [HTTP]",
    "添加 [TUIC]",
    "添加 [AnyTLS]",
    "添加 [Naive]",
    "左右分栏",
    "上下分栏",
    "标签页",
    "主窗布局",
    "快速操作",
    // The Overview hero's System Proxy value, reached as `t(PROXY_VALUE[mode])`.
    // It is deliberately shorter than the popover's `PROXY_LABEL`, which the
    // hero already labels "系统代理" — so these have no literal `t("…")` to find.
    "自动配置",
    "清除",
    "不改变",
  ];
}

/** Simplified Chinese is the source, so it is translated by definition. */
const SOURCE_LOCALE: Locale = "zh-Hans";

describe("translate", () => {
  it("opens in English", () => {
    expect(DEFAULT_LOCALE).toBe("en");
    expect(translate(DEFAULT_LOCALE, "连接")).toBe("Connect");
  });

  it("returns the source string in the language it is written in", () => {
    expect(translate(SOURCE_LOCALE, "连接")).toBe("连接");
  });

  it("falls back to the source string rather than to nothing", () => {
    // A missing translation has to be visible: an empty label would hide the
    // control it belongs to.
    expect(translate("de", "这句还没有翻译")).toBe("这句还没有翻译");
  });

  it("falls back for a language this build does not know", () => {
    // A stored setting from a newer build must not take the window down.
    expect(translate("kl" as Locale, "连接")).toBe("连接");
  });
});

describe("coverage", () => {
  it("finds the strings the components use", () => {
    // Guards the extraction itself: a regex that matched nothing would make
    // every coverage assertion below pass for the wrong reason.
    expect(usedKeys().length).toBeGreaterThan(150);
  });

  it.each(LOCALES.filter((locale) => locale.id !== SOURCE_LOCALE))(
    "$label translates every string the components use",
    ({ id }) => {
      expect(untranslated(id, usedKeys())).toEqual([]);
    },
  );

  it.each(LOCALES.filter((locale) => locale.id !== SOURCE_LOCALE))(
    "$label carries nothing the components no longer use",
    ({ id }) => {
      // A stale entry is not harmful, but it is a translation nobody will ever
      // see and a reviewer's time spent on it is wasted.
      const used = new Set(usedKeys());
      expect(translatedKeys(id).filter((key) => !used.has(key))).toEqual([]);
    },
  );

  it("offers only languages that are fully translated", () => {
    // The picker is the promise. A language listed there and half translated
    // shows a user a window in two languages at once. The source language is
    // exempt: its dictionary is empty because every lookup is already the
    // answer.
    for (const { id } of LOCALES.filter(({ id }) => id !== SOURCE_LOCALE)) {
      expect(untranslated(id, usedKeys())).toEqual([]);
    }
  });
});
