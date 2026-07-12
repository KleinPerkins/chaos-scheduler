import { Fragment } from "react";
import type { CSSProperties } from "react";
import { formatDuration } from "../../lib/duration";
import Vehicle from "./Vehicle";
import type { VehicleColor, VehicleStyle } from "./Vehicle";
import "./RaceTrack.css";

export interface RaceTrackJob {
  /** Job name — the lane's primary label. */
  job: string;
  /** Agent / worker running the job. Folded into the accessible summary. */
  agent?: string;
  /** Elapsed time so far, in seconds — drives the car's position. */
  elapsedSeconds: number;
  /** Expected (P50) runtime, in seconds — the race length (finish line). */
  expectedSeconds: number;
  /** Override the lane's vehicle color; defaults to a per-lane cycle. */
  color?: VehicleColor;
}

export interface RaceTrackProps {
  /** Running jobs, one lane each (Treatment A). Drawn top-to-bottom in order. */
  jobs: RaceTrackJob[];
  /** Card title. Pass `null` to omit. Defaults to the Figma master's title. */
  title?: string | null;
  /**
   * Enable the subtle idle "rev" motion on the cars. Off by default so
   * screenshots are deterministic; even when on it is CSS-keyframe based and
   * gated behind `prefers-reduced-motion`.
   */
  animate?: boolean;
  /** Accessible summary; auto-generated from the jobs when omitted. */
  ariaLabel?: string;
  /** Extra class(es) merged onto the root `<svg>`. */
  className?: string;
}

// --- Fixed layout, in SVG user units, matching the Figma master (527:4262). ---
const VIEW_W = 460;
const LANE_TOP0 = 47; // top of the first lane's track
const LANE_PITCH = 40; // vertical distance between lanes
const TRACK_H = 22;
const BOTTOM_PAD = 21;
const TRACK_LEFT = 119;
const TRACK_W = 316;
const TRACK_RIGHT = TRACK_LEFT + TRACK_W; // 435
const START_X = 121;
const FINISH_FRAC = 0.78; // finish line at 78% of the track interior
const FINISH_X = TRACK_LEFT + FINISH_FRAC * TRACK_W; // ≈ 365.48
const RACING_LINE_X1 = 123;
const RACING_LINE_X2 = FINISH_X - 6;
const OVERTIME_MAX = 2; // a car at ≥200% of expected parks at the track's end
const TRACK_R = 8;
const CARD_R = 12;

const DEFAULT_TITLE = "Running — vehicle lanes (% of expected)";
// Per-lane color cycle when a job doesn't pin its own color.
const LANE_COLORS: VehicleColor[] = ["blue", "teal", "amber"];

/**
 * Vehicle class by expected runtime: quick jobs get a nimble sedan, the
 * longest-haul jobs a truck (faithful to the Figma race view — sedan ≤ 12m,
 * coupe ≤ 24m, racer ≤ 40m, else truck).
 */
function vehicleStyleForExpected(expectedSeconds: number): VehicleStyle {
  const minutes = expectedSeconds / 60;
  if (minutes <= 12) return "sedan";
  if (minutes <= 24) return "coupe";
  if (minutes <= 40) return "racer";
  return "truck";
}

// Center-x of the car for a given progress (elapsed / expected). Runs from the
// start line to the finish over [0,1], then eases into the overtime zone.
function carCenterX(progress: number): number {
  const p = Math.max(0, progress);
  if (p <= 1) return START_X + p * (FINISH_X - START_X);
  const overFrac = Math.min((p - 1) / (OVERTIME_MAX - 1), 1);
  return FINISH_X + overFrac * (TRACK_RIGHT - FINISH_X);
}

// A rectangle rounded only on its right corners (the overtime zone caps the
// track's rounded right end but is square where it meets the finish line).
function roundedRightRect(
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
): string {
  return `M${x},${y} H${x + w - r} A${r},${r} 0 0 1 ${x + w},${y + r} V${y + h - r} A${r},${r} 0 0 1 ${x + w - r},${y + h} H${x} Z`;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

const RACING_LINE_STYLE: CSSProperties = { stroke: "var(--text-secondary)" };
const START_STYLE: CSSProperties = { fill: "var(--text-secondary)" };
const TRACK_STYLE: CSSProperties = { fill: "var(--bg-tertiary)" };
const OVERTIME_STYLE: CSSProperties = { fill: "var(--error)" };
const FLAG_BASE_STYLE: CSSProperties = { fill: "var(--text-primary)" };
const FLAG_CHECK_STYLE: CSSProperties = { fill: "var(--bg-secondary)" };

// Checkered finish flag: a 6-wide, full-track-height bar with a 2×4 checker.
function FinishFlag({ laneTop }: { laneTop: number }) {
  const flagX = FINISH_X - 3;
  const rows = [0, 1, 2, 3];
  const cols = [0, 1];
  return (
    <>
      <rect
        x={flagX}
        y={laneTop}
        width={6}
        height={TRACK_H}
        style={FLAG_BASE_STYLE}
      />
      {cols.map((col) =>
        rows.map((row) =>
          (col + row) % 2 === 0 ? (
            <rect
              key={`${col}-${row}`}
              x={flagX + col * 3}
              y={laneTop + row * 5.5}
              width={3}
              height={5.5}
              style={FLAG_CHECK_STYLE}
            />
          ) : null,
        ),
      )}
    </>
  );
}

/**
 * Race-view of running jobs — one lane per job (Treatment A). Each lane's track
 * length is the job's expected runtime, so the car position is elapsed/expected;
 * a car past the checkered finish line sits in the red overtime zone. The car's
 * silhouette is chosen from its expected runtime and it turns status-red once
 * overrunning. Presentational and props-driven (all data via props, no
 * data-fetching); colors bind to tokens (never raw hex) and it renders in
 * dark/light. Responsive: fills its container at a fixed aspect ratio. Mirrors
 * the Figma `RaceTrack` master (node 527:4262).
 */
export default function RaceTrack({
  jobs,
  title = DEFAULT_TITLE,
  animate = false,
  ariaLabel,
  className,
}: RaceTrackProps) {
  const height =
    jobs.length > 0
      ? LANE_TOP0 + (jobs.length - 1) * LANE_PITCH + TRACK_H + BOTTOM_PAD
      : 90;

  const summary =
    ariaLabel ??
    (jobs.length === 0
      ? "No running jobs"
      : `Running jobs versus expected runtime: ${jobs
          .map((j) => {
            const over = j.elapsedSeconds > j.expectedSeconds;
            const who = j.agent ? ` on ${j.agent}` : "";
            return `${j.job}${who} ${formatDuration(j.elapsedSeconds * 1000)} of about ${formatDuration(
              j.expectedSeconds * 1000,
            )}${over ? " (overrunning)" : ""}`;
          })
          .join("; ")}`);

  return (
    <svg
      className={[
        "cs-racetrack",
        animate ? "cs-racetrack--animated" : null,
        className,
      ]
        .filter(Boolean)
        .join(" ")}
      viewBox={`0 0 ${VIEW_W} ${height}`}
      style={{
        width: "100%",
        aspectRatio: `${VIEW_W} / ${height}`,
        display: "block",
      }}
      role="img"
      aria-label={summary}
    >
      <rect
        x={0.5}
        y={0.5}
        width={VIEW_W - 1}
        height={height - 1}
        rx={CARD_R}
        style={{ fill: "var(--bg-secondary)", stroke: "var(--border)" }}
      />

      {title ? (
        <text className="cs-racetrack__title" x={15} y={19}>
          {title}
        </text>
      ) : null}

      {/* Legend: expected (mini checkered flag) + overtime (red swatch). */}
      <rect x={285} y={14} width={8} height={8} style={FLAG_BASE_STYLE} />
      <rect x={285} y={14} width={4} height={4} style={FLAG_CHECK_STYLE} />
      <rect x={289} y={18} width={4} height={4} style={FLAG_CHECK_STYLE} />
      <text className="cs-racetrack__legend" x={297} y={19}>
        expected
      </text>
      <rect
        x={355}
        y={14}
        width={8}
        height={8}
        rx={2}
        style={OVERTIME_STYLE}
        fillOpacity={0.55}
      />
      <text className="cs-racetrack__legend" x={367} y={19}>
        overtime
      </text>

      {jobs.length === 0 ? (
        <text className="cs-racetrack__empty" x={VIEW_W / 2} y={64}>
          No running jobs
        </text>
      ) : (
        jobs.map((j, i) => {
          const laneTop = LANE_TOP0 + i * LANE_PITCH;
          const laneCenter = laneTop + TRACK_H / 2;
          const expected = Math.max(0, j.expectedSeconds);
          const progress = expected > 0 ? j.elapsedSeconds / expected : 0;
          const over = j.elapsedSeconds > expected && expected > 0;
          const vStyle = vehicleStyleForExpected(expected);
          const vColor = j.color ?? LANE_COLORS[i % LANE_COLORS.length];
          const vw = vStyle === "truck" ? 60 : 36;
          const vh = vStyle === "truck" ? 18 : 16;
          const vx = clamp(
            carCenterX(progress) - vw / 2,
            TRACK_LEFT,
            TRACK_RIGHT - vw,
          );
          const vy = laneTop + (TRACK_H - vh) / 2;

          return (
            <Fragment key={`${j.job}-${i}`}>
              <text className="cs-racetrack__lane" x={15} y={laneTop + 6}>
                {j.job}
              </text>
              <text
                className={[
                  "cs-racetrack__sub",
                  over ? "cs-racetrack__sub--over" : null,
                ]
                  .filter(Boolean)
                  .join(" ")}
                x={15}
                y={laneTop + 18}
              >
                {formatDuration(j.elapsedSeconds * 1000)} / ~
                {formatDuration(expected * 1000)}
              </text>

              <rect
                x={TRACK_LEFT}
                y={laneTop}
                width={TRACK_W}
                height={TRACK_H}
                rx={TRACK_R}
                style={TRACK_STYLE}
              />
              <path
                d={roundedRightRect(
                  FINISH_X,
                  laneTop,
                  TRACK_RIGHT - FINISH_X,
                  TRACK_H,
                  TRACK_R,
                )}
                style={OVERTIME_STYLE}
                fillOpacity={0.25}
              />
              <line
                x1={RACING_LINE_X1}
                y1={laneCenter}
                x2={RACING_LINE_X2}
                y2={laneCenter}
                style={RACING_LINE_STYLE}
                strokeWidth={1.5}
                strokeDasharray="6 6"
              />
              <rect
                x={START_X}
                y={laneTop + 2}
                width={2}
                height={18}
                rx={1}
                style={START_STYLE}
              />
              <FinishFlag laneTop={laneTop} />

              <g transform={`translate(${vx} ${vy})`}>
                <g
                  className="cs-racetrack__vehicle"
                  style={{ animationDelay: `${(i % 3) * 0.15}s` }}
                >
                  <Vehicle
                    style={vStyle}
                    color={vColor}
                    over={over}
                    decorative
                  />
                </g>
              </g>
            </Fragment>
          );
        })
      )}
    </svg>
  );
}
