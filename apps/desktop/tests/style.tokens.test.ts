import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

const css = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "../src/style.css"),
  "utf8",
);

function token(source: string, name: string): string {
  const match = source.match(new RegExp(`${name}:\\s*([^;]+);`));
  if (!match) {
    throw new Error(`no ${name} in block`);
  }
  return (match[1] ?? "").trim().toLowerCase().replace(/\s+/g, "");
}

describe("handoff tokens and shell numbers", () => {
  it("locks the light and dark color table and 212 / 48 / 28 layout", () => {
    const darkAt = css.indexOf('[data-theme="dark"]');
    if (darkAt < 0) {
      throw new Error("no dark theme block");
    }
    const light = css.slice(0, darkAt);
    const dark = css.slice(darkAt, css.indexOf("\n}", darkAt) + 2);

    expect(token(light, "--c-bg")).toBe("#eef0f4");
    expect(token(light, "--c-side")).toBe("#e8ebf0");
    expect(token(light, "--c-surf")).toBe("#ffffff");
    expect(token(light, "--c-surf2")).toBe("#f5f7fa");
    expect(token(light, "--c-line")).toBe("#e2e6ec");
    expect(token(light, "--c-line2")).toBe("#ced4dd");
    expect(token(light, "--c-text")).toBe("#10151b");
    expect(token(light, "--c-mut")).toBe("#5f6975");
    expect(token(light, "--c-mut2")).toBe("#8a94a1");
    expect(token(light, "--c-acc")).toBe("#0b8fa3");
    expect(token(light, "--c-acc-b")).toBe("#0a7f91");
    expect(token(light, "--c-acc-t")).toBe("#e4f4f7");
    expect(token(light, "--c-ok")).toBe("#1f9d53");
    expect(token(light, "--c-ok-t")).toBe("#e6f5ec");
    expect(token(light, "--c-warn")).toBe("#b57614");
    expect(token(light, "--c-warn-t")).toBe("#fcf3e2");
    expect(token(light, "--c-dang")).toBe("#d0392b");
    expect(token(light, "--c-dang-t")).toBe("#fcebe9");
    expect(token(light, "--c-blue")).toBe("#2f6fe4");
    expect(token(light, "--c-hover")).toBe("#f0f3f7");
    expect(token(light, "--c-sel")).toBe("#e4f4f7");
    expect(token(light, "--c-track")).toBe("#d7dce4");

    expect(token(dark, "--c-bg")).toBe("#101318");
    expect(token(dark, "--c-side")).toBe("#0b0e12");
    expect(token(dark, "--c-surf")).toBe("#171b21");
    expect(token(dark, "--c-surf2")).toBe("#1d222a");
    expect(token(dark, "--c-line")).toBe("#262c35");
    expect(token(dark, "--c-line2")).toBe("#333b46");
    expect(token(dark, "--c-text")).toBe("#e9edf2");
    expect(token(dark, "--c-mut")).toBe("#98a2af");
    expect(token(dark, "--c-mut2")).toBe("#6d7784");
    expect(token(dark, "--c-acc")).toBe("#22c0d4");
    expect(token(dark, "--c-acc-b")).toBe("#3acfe2");
    expect(token(dark, "--c-ok")).toBe("#3fb950");
    expect(token(dark, "--c-warn")).toBe("#d6a02b");
    expect(token(dark, "--c-dang")).toBe("#f2604a");
    expect(token(dark, "--c-blue")).toBe("#5b8def");
    expect(token(dark, "--c-hover")).toBe("#1f252e");
    expect(token(dark, "--c-track")).toBe("#333b46");

    expect(css).toMatch(
      /\.app-shell\s*\{[^}]*grid-template-columns:\s*212px\s+minmax\(0,\s*1fr\)/s,
    );
    expect(css).toMatch(
      /\.app-shell\s*\{[^}]*grid-template-rows:\s*minmax\(0,\s*1fr\)\s+28px/s,
    );
    expect(css).toMatch(/\.app-topbar\s*\{[^}]*height:\s*48px/s);
    expect(css).toMatch(/\.app-statusbar\s*\{[^}]*height:\s*28px/s);
  });

  /*
   * `.form-grid > label { display: contents }` splits every label into its text
   * and its control, so each one consumes two cells. An odd column count shifts
   * the pairing by one on every row and walks each label away from its field —
   * which is what shipped in 1.1.0, scrambling the add-node form.
   */
  it("keeps the form grid on an even column count", () => {
    const at = css.indexOf(".form-grid {");
    expect(at).toBeGreaterThan(-1);
    const block = css.slice(at, css.indexOf("}", at));
    const columns = /grid-template-columns:\s*([^;]+);/.exec(block)?.[1] ?? "";
    expect(columns).not.toBe("");

    // Count top-level tracks: a space separates tracks only outside minmax(...).
    let depth = 0;
    let tracks = 1;
    for (const character of columns.trim().replace(/\s+/gu, " ")) {
      if (character === "(") depth += 1;
      else if (character === ")") depth -= 1;
      else if (character === " " && depth === 0) tracks += 1;
    }

    expect(tracks % 2).toBe(0);
    // `.sr-only` must stay out of it: `display: contents` drops the label's box
    // and with it the clip that hides a screen-reader-only control.
    expect(css).toMatch(
      /\.form-grid\s*>\s*label:not\(\.checkbox-label\):not\(\.sr-only\)\s*\{[^}]*display:\s*contents/s,
    );

    // Parity also needs every non-pair child to take a whole row: a chip row or
    // a hint occupying one cell shifts each later pair by one, which is the
    // second half of the same 1.1.0 bug.
    expect(css).toMatch(
      /\.form-grid\s*>\s*\*:not\(label\),\s*\.form-grid\s*>\s*label\.checkbox-label\s*\{[^}]*grid-column:\s*1\s*\/\s*-1/s,
    );
  });
});
