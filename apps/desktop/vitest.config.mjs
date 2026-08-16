import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    coverage: {
      exclude: [
        "src/main.tsx",
        // Relocated dialog markup: behaviour is covered via App.test.tsx.
        "src/components/overlays/AppDialogs.tsx",
        // Protocol-field display with many optional branches; opened from App tests.
        "src/components/nodes/NodeInspector.tsx",
        "src/components/CommandPalette.tsx",
        // In-page settings/routing/DNS forms: command wiring is covered by App.test.tsx.
        "src/pages/SettingsPage.tsx",
        "src/pages/RoutingPage.tsx",
        "src/pages/DnsPage.tsx",
      ],
      include: ["src/**/*.ts", "src/**/*.tsx"],
      provider: "v8",
      reporter: ["text"],
      thresholds: {
        branches: 80,
        functions: 80,
        lines: 80,
        statements: 80,
      },
    },
    // The shell tests drive the whole app through many `act` flushes. The
    // macos-x86_64 runner takes roughly six times the local wall clock and
    // overran the 5s default; this is headroom for a slow runner, not a
    // licence for a slow test.
    testTimeout: 20000,
  },
});
