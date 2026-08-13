import { describe, expect, it } from "vitest";

// Vite inlines the file at build time, so the test needs no filesystem types
// and reads exactly what the bundler sees.
import appSource from "./App.tsx?raw";

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
  return [...appSource.matchAll(/\bt\("((?:[^"\\]|\\.)*)"\)/g)].map(
    (match) => match[1],
  );
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
