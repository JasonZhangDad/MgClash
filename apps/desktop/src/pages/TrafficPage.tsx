import {
  formatBytes,
  formatRate,
  groupTraffic,
  processTraffic,
} from "../appHelpers";
import type { AppModel } from "../hooks/useAppController";
import { Sparkline } from "../components/traffic/Sparkline";
import { Card, StatCard } from "../components/ui/Ui";

function RankList({
  items,
  label,
}: {
  items: { name: string; value: number }[];
  label?: string;
}) {
  const max = Math.max(1, ...items.map((item) => item.value));
  return (
    <ul className="rank-list" aria-label={label}>
      {items.map((item) => (
        <li key={item.name}>
          <div>
            <span>{item.name}</span>
            <span className="mono muted">{formatBytes(item.value)}</span>
          </div>
          <div className="rank-bar">
            <i style={{ width: `${(item.value / max) * 100}%` }} />
          </div>
        </li>
      ))}
    </ul>
  );
}

function DailyHistoryChart({
  days,
  t,
}: {
  days: { bytes: number; day: string }[];
  t: (text: string) => string;
}) {
  const width = 600;
  const height = 120;
  const pad = 8;
  const max = Math.max(1, ...days.map((day) => day.bytes));
  const points = days
    .map((day, index) => {
      const x =
        days.length === 1 ? width / 2 : (index / (days.length - 1)) * width;
      const y = pad + (height - pad * 2) - (day.bytes / max) * (height - pad * 2);
      return `${x},${y}`;
    })
    .join(" ");

  return (
    <div className="daily-history" aria-label={t("历史流量统计")}>
      <header className="card-head">
        <strong>{t("历史流量统计")}</strong>
        <span className="muted">{t("按日聚合")}</span>
      </header>
      {days.length === 0 ? (
        <p className="muted">{t("暂无按日流量记录")}</p>
      ) : (
        <>
          <svg
            viewBox={`0 0 ${width} ${height}`}
            preserveAspectRatio="none"
            height={height}
            aria-hidden
          >
            <polyline className="spark-down" points={points} fill="none" />
          </svg>
          <div className="spark-ticks">
            {days.map((day) => (
              <span key={day.day}>{day.day}</span>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

export function TrafficPage({ app }: { app: AppModel }) {
  const { t, traffic, trafficHistory, connections, nodes, nodeTraffic, nodeGroups } =
    app;
  const processes = processTraffic(connections?.connections ?? []).map((item) => ({
    name: item.name,
    value: item.downloadBytes + item.uploadBytes,
  }));
  const byNode = nodes
    .map((node) => {
      const entry = nodeTraffic[node.id];
      return {
        name: node.name,
        value:
          (entry?.todayDownloadBytes ?? 0) + (entry?.todayUploadBytes ?? 0),
      };
    })
    .sort((left, right) => right.value - left.value)
    .slice(0, 8);
  const byGroup = nodeGroups
    .map((group) => {
      const entry = groupTraffic(nodes, nodeTraffic, group.id);
      return {
        name: group.name,
        value: entry.todayDownloadBytes + entry.todayUploadBytes,
      };
    })
    .sort((left, right) => right.value - left.value);

  return (
    <div className="page-pad">
      <div className="stat-grid">
        <StatCard
          label={t("上传速率")}
          value={formatRate(traffic.uploadBytesPerSecond)}
        />
        <StatCard
          label={t("下载速率")}
          value={formatRate(traffic.downloadBytesPerSecond)}
        />
        <StatCard label={t("今日流量")} value={formatBytes(traffic.todayBytes)} />
        <StatCard label={t("累计流量")} value={formatBytes(traffic.totalBytes)} />
      </div>
      <Card>
        <header className="card-head">
          <strong>{t("实时流量")}</strong>
          <span className="muted">{t("最近 60 秒")} · {t("每秒采样")}</span>
          <span className="spark-legend">
            <i className="lg-down" /> {t("下载")} {formatRate(traffic.downloadBytesPerSecond)}
            <i className="lg-up" /> {t("上传")} {formatRate(traffic.uploadBytesPerSecond)}
          </span>
        </header>
        <Sparkline
          samples={trafficHistory}
          height={178}
          ticks={["-60s", "-45s", "-30s", "-15s", "now"]}
        />
      </Card>
      <div className="rank-grid">
        <Card>
          <header className="card-head">
            <strong>{t("进程流量")}</strong>
            <span className="muted">{t("仅统计当前打开的连接")}</span>
          </header>
          <RankList items={processes} label={t("进程流量")} />
        </Card>
        <Card>
          <header className="card-head">
            <strong>{t("节点")}</strong>
            <span className="muted">By node</span>
          </header>
          <RankList items={byNode} />
        </Card>
        <Card>
          <header className="card-head">
            <strong>{t("代理组流量")}</strong>
            <span className="muted">By group</span>
          </header>
          <RankList items={byGroup} />
        </Card>
      </div>
      <Card>
        <DailyHistoryChart days={traffic.daily ?? []} t={t} />
      </Card>
    </div>
  );
}