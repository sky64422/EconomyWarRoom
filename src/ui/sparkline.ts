import type { SparklinePoint } from "./types";

export interface SparklinePaths {
  /** Stroke path (line only). */
  line: string;
  /** Closed path under the line for gradient fill. */
  area: string;
}

/** Build SVG line + area path `d` strings from sparkline close prices. */
export function sparklinePaths(
  points: SparklinePoint[],
  width: number,
  height: number,
  padding = 1,
): SparklinePaths {
  if (points.length === 0) return { line: "", area: "" };
  const closes = points.map((p) => p.close);
  const min = Math.min(...closes);
  const max = Math.max(...closes);
  const span = max - min || 1;
  const w = width - padding * 2;
  const h = height - padding * 2;
  const coords = points.map((p, i) => {
    const x = padding + (i / Math.max(points.length - 1, 1)) * w;
    const y = padding + h - ((p.close - min) / span) * h;
    return { x, y };
  });
  const line = coords
    .map((c, i) => `${i === 0 ? "M" : "L"}${c.x.toFixed(2)} ${c.y.toFixed(2)}`)
    .join(" ");
  const first = coords[0];
  const last = coords[coords.length - 1];
  const baseY = (padding + h).toFixed(2);
  const area = `${line} L${last.x.toFixed(2)} ${baseY} L${first.x.toFixed(2)} ${baseY} Z`;
  return { line, area };
}

/** @deprecated Prefer {@link sparklinePaths}; kept for call sites that only need the line. */
export function sparklinePath(
  points: SparklinePoint[],
  width: number,
  height: number,
  padding = 1,
): string {
  return sparklinePaths(points, width, height, padding).line;
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
