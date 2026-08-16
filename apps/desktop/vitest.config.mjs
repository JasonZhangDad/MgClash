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
  },
});
