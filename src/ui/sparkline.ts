import type { SparklinePoint } from "./types";

/** Build an SVG path `d` string from sparkline close prices. */
export function sparklinePath(
  points: SparklinePoint[],
  width: number,
  height: number,
  padding = 1,
): string {
  if (points.length === 0) return "";
  const closes = points.map((p) => p.close);
  const min = Math.min(...closes);
  const max = Math.max(...closes);
  const span = max - min || 1;
  const w = width - padding * 2;
  const h = height - padding * 2;
  return points
    .map((p, i) => {
      const x = padding + (i / Math.max(points.length - 1, 1)) * w;
      const y = padding + h - ((p.close - min) / span) * h;
      return `${i === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" ");
}

/** Stroke color class for a sparkline based on first vs last close. */
export function sparklineTone(points: SparklinePoint[]): "up" | "down" | "flat" {
  if (points.length < 2) return "flat";
  const first = points[0].close;
  const last = points[points.length - 1].close;
  if (last > first) return "up";
  if (last < first) return "down";
  return "flat";
}
