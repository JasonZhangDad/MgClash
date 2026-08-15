import { describe, expect, it } from "vitest";

// Vite inlines the files at build time, so the test needs no filesystem types
// and reads exactly what the bundler sees. The v2rayN-style shell splits what
// used to be one file across App.tsx and its components, so every file that
// can call `t(...)` has to be scanned, not just App.tsx.
import appSource from "./App.tsx?raw";
import appHelpersSource from "./appHelpers.ts?raw";
import cameraScannerSource from "./components/CameraScanner.tsx?raw";
import connectionsViewSource from "./components/ConnectionsView.tsx?raw";
import nodeContextMenuSource from "./components/NodeContextMenu.tsx?raw";
import proxiesViewSource from "./components/ProxiesView.tsx?raw";
import dialogSource from "./components/Dialog.tsx?raw";
import menuBarSource from "./components/MenuBar.tsx?raw";
import msgViewSource from "./components/MsgView.tsx?raw";
import statusBarSource from "./components/StatusBar.tsx?raw";

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
    appHelpersSource,
    cameraScannerSource,
    connectionsViewSource,
    nodeContextMenuSource,
    proxiesViewSource,
    menuBarSource,
    statusBarSource,
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
    // Column titles passed to SortableHeader as `label`, then translated
    // there as `t(label)` — the regex above cannot see the string.
    "传输协议",
    "订阅分组",
    "别名",
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
