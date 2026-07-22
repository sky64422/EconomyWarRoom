import type { AssetKind, SparklinePoint } from "./types";

export interface SparklinePaths {
  /** Stroke path for the sparkline line. */
  line: string;
  /** Closed area path under the line. */
  area: string;
  /** Dashed baseline path, if available. */
  baseline: string;
  /** SVG height used by the gradient. */
  height: number;
}

export interface SparklineSvgOptions {
  id: string;
  assetKind: AssetKind;
  stroke: string;
  progress?: number | null;
}

const NY_TIME_ZONE = "America/New_York";
const WEEKDAY_TO_INDEX: Record<string, number> = {
  Sun: 0,
  Mon: 1,
  Tue: 2,
  Wed: 3,
  Thu: 4,
  Fri: 5,
  Sat: 6,
};

/**
 * Build SVG line + area path strings from sparkline close prices.
 *
 * When [baselineValue] is provided, it is included in the chart range and the
 * returned baseline path aligns to that value, leaving space above and below
 * like the Flutter reference card.
 */
export function sparklinePaths(
  points: SparklinePoint[],
  width: number,
  height: number,
  padding = 1,
  baselineValue?: number | null,
): SparklinePaths {
  if (points.length === 0) {
    return { line: "", area: "", baseline: "", height };
  }

  const closes = points.map((p) => p.close);
  const validBaseline =
    baselineValue != null && Number.isFinite(baselineValue)
      ? baselineValue
      : null;

  const pointMin = Math.min(...closes);
  const pointMax = Math.max(...closes);
  const pointRange = pointMax - pointMin;
  const yPad = pointRange > 0 ? pointRange * 0.15 : 1.0;

  let chartMinY = pointMin - yPad;
  let chartMaxY = pointMax + yPad;

  if (validBaseline != null) {
    if (validBaseline < chartMinY) chartMinY = validBaseline - yPad;
    if (validBaseline > chartMaxY) chartMaxY = validBaseline + yPad;

    const spaceAbove = chartMaxY - validBaseline;
    const spaceBelow = validBaseline - chartMinY;
    if (spaceAbove < spaceBelow * 0.6) {
      chartMaxY = validBaseline + spaceBelow * 0.6;
    }
    if (spaceBelow < spaceAbove * 0.6) {
      chartMinY = validBaseline - spaceAbove * 0.6;
    }
  }

  const yRange = Math.max(chartMaxY - chartMinY, 0.0001);
  const plotWidth = Math.max(width - padding * 2, 1);
  const plotHeight = Math.max(height - padding * 2, 1);

  const coords = points.map((p, i) => {
    const x = padding + (i / Math.max(points.length - 1, 1)) * plotWidth;
    const y =
      padding + plotHeight - ((p.close - chartMinY) / yRange) * plotHeight;
    return { x, y };
  });

  const line = coords
    .map((c, i) => `${i === 0 ? "M" : "L"}${c.x.toFixed(2)} ${c.y.toFixed(2)}`)
    .join(" ");

  const first = coords[0];
  const last = coords[coords.length - 1];
  const baselineY = validBaseline != null
    ? padding + plotHeight - ((validBaseline - chartMinY) / yRange) * plotHeight
    : height - padding;
  const baseline = `M${padding.toFixed(2)} ${baselineY.toFixed(2)} L${(width - padding).toFixed(2)} ${baselineY.toFixed(2)}`;
  const area = `${line} L${last.x.toFixed(2)} ${baselineY.toFixed(2)} L${first.x.toFixed(2)} ${baselineY.toFixed(2)} Z`;

  return { line, area, baseline, height };
}

/** @deprecated Prefer {@link sparklinePaths}. */
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

export function sparklineProgress(
  points: SparklinePoint[],
  assetKind: AssetKind,
  now: Date = new Date(),
): number | null {
  if (assetKind === "crypto" || points.length < 2) {
    return null;
  }

  const current = zonedParts(now, NY_TIME_ZONE);
  if (current.weekday < 1 || current.weekday > 5) {
    return null;
  }

  const last = zonedParts(new Date(points[points.length - 1].t * 1000), NY_TIME_ZONE);
  if (
    current.year !== last.year ||
    current.month !== last.month ||
    current.day !== last.day
  ) {
    return null;
  }

  const openMinutes = 9 * 60 + 30;
  const closeMinutes = 16 * 60;
  const currentMinutes = current.hour * 60 + current.minute + current.second / 60;
  if (currentMinutes < openMinutes || currentMinutes > closeMinutes) {
    return null;
  }

  const progress = (currentMinutes - openMinutes) / (closeMinutes - openMinutes);
  return Math.max(0, Math.min(1, progress));
}

export function sparklineSvgMarkup(
  points: SparklinePoint[],
  width: number,
  height: number,
  options: SparklineSvgOptions,
  baselineValue?: number | null,
): string {
  void options.assetKind;
  const { line, area, baseline } = sparklinePaths(
    points,
    width,
    height,
    1,
    baselineValue,
  );
  if (!line) return "";

  const gradientId = `${options.id}-gradient`;
  const clipId = `${options.id}-clip`;
  const progress = options.progress;
  const normalizedProgress =
    progress == null || !Number.isFinite(progress)
      ? null
      : Math.max(0, Math.min(1, progress));
  const clipWidth = normalizedProgress == null ? width : width * normalizedProgress;
  const animate = normalizedProgress != null && normalizedProgress < 1;
  const dashOffset = animate ? (1 - normalizedProgress) * 100 : 0;

  return `
    <defs>
      <linearGradient id="${gradientId}" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="0" y2="${height}">
        <stop offset="0%" stop-color="${options.stroke}" stop-opacity="0.62" />
        <stop offset="45%" stop-color="${options.stroke}" stop-opacity="0.34" />
        <stop offset="100%" stop-color="${options.stroke}" stop-opacity="0.08" />
      </linearGradient>
      ${
        animate
          ? `<clipPath id="${clipId}" clipPathUnits="userSpaceOnUse">
              <rect x="0" y="0" width="${clipWidth.toFixed(2)}" height="${height}" />
            </clipPath>`
          : ""
      }
    </defs>
    <path d="${baseline}" fill="none" stroke="${options.stroke}" stroke-opacity="0.32" stroke-width="1" stroke-dasharray="3 3" stroke-linecap="round" />
    <g ${animate ? `clip-path="url(#${clipId})"` : ""}>
      <path d="${area}" fill="url(#${gradientId})" stroke="none" />
      <path d="${line}" fill="none" stroke="${options.stroke}" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" ${animate ? 'pathLength="100" stroke-dasharray="100" stroke-dashoffset="' + dashOffset.toFixed(2) + '"' : ""} />
    </g>
  `;
}

function zonedParts(date: Date, timeZone: string): {
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
  second: number;
  weekday: number;
} {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone,
    weekday: "short",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
  }).formatToParts(date);

  const get = (type: string): string => parts.find((part) => part.type === type)?.value ?? "";
  const weekday = WEEKDAY_TO_INDEX[get("weekday")] ?? 0;
  return {
    year: Number(get("year")),
    month: Number(get("month")),
    day: Number(get("day")),
    hour: Number(get("hour")),
    minute: Number(get("minute")),
    second: Number(get("second")),
    weekday,
  };
}
