import type { TrafficSample } from "../../appHelpers";

export function Sparkline({
  samples,
  height = 158,
  ticks,
}: {
  height?: number;
  samples: TrafficSample[];
  ticks: string[];
}) {
  const width = 600;
  const pad = 6;
  const innerH = height - pad * 2;
  const max = Math.max(
    1,
    ...samples.flatMap((sample) => [sample.up, sample.down]),
  );
  const points = (key: "up" | "down") => {
    if (samples.length === 0) {
      return `0,${height - pad}`;
    }
    return samples
      .map((sample, index) => {
        const x =
          samples.length === 1
            ? width / 2
            : (index / (samples.length - 1)) * width;
        const y = pad + innerH - (sample[key] / max) * innerH;
        return `${x},${y}`;
      })
      .join(" ");
  };

  return (
    <div className="sparkline">
      <svg
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="none"
        height={height}
        aria-hidden
      >
        <line x1="0" y1={pad} x2={width} y2={pad} className="spark-grid" />
        <line
          x1="0"
          y1={height / 2}
          x2={width}
          y2={height / 2}
          className="spark-grid"
        />
        <line
          x1="0"
          y1={height - pad}
          x2={width}
          y2={height - pad}
          className="spark-grid"
        />
        <polyline className="spark-down" points={points("down")} />
        <polyline className="spark-up" points={points("up")} />
      </svg>
      <div className="spark-ticks">
        {ticks.map((tick) => (
          <span key={tick}>{tick}</span>
        ))}
      </div>
    </div>
  );
}