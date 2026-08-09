import { useEffect, useState } from "react";

import { loadPlatformSummary, type PlatformSummary } from "./platform";

type PlatformState =
  | { status: "loading" }
  | { status: "ready"; summary: PlatformSummary }
  | { message: string; status: "error" };

export default function App() {
  const [platform, setPlatform] = useState<PlatformState>({ status: "loading" });

  useEffect(() => {
    loadPlatformSummary()
      .then((summary) => setPlatform({ status: "ready", summary }))
      .catch((error: unknown) =>
        setPlatform({ message: String(error), status: "error" }),
      );
  }, []);

  return (
    <main className="app-shell">
      <header className="toolbar">
        <h1>MgClash</h1>
      </header>

      <section className="content">
        <h2>运行环境</h2>

        {platform.status === "loading" && <p>正在读取 Rust 平台信息…</p>}

        {platform.status === "ready" && (
          <dl>
            <div>
              <dt>构建目标</dt>
              <dd>{platform.summary.artifactIdentifier}</dd>
            </div>
            <div>
              <dt>Unsigned TUN</dt>
              <dd>{platform.summary.tunAvailability}</dd>
            </div>
          </dl>
        )}

        {platform.status === "error" && (
          <p className="error" role="alert">
            无法读取平台信息：{platform.message}
          </p>
        )}
      </section>
    </main>
  );
}
