import { useCallback, useId, useState } from "react";
import Select from "./Select";
import "./ScheduleBuilder.css";

/* eslint-disable react-refresh/only-export-components */

type Frequency = "hourly" | "daily" | "weekdays" | "weekly" | "monthly";

interface Props {
  value: string;
  onChange: (cron: string) => void;
  timezone?: string;
}

const DAYS_OF_WEEK = [
  { key: "Mon", label: "M" },
  { key: "Tue", label: "T" },
  { key: "Wed", label: "W" },
  { key: "Thu", label: "T" },
  { key: "Fri", label: "F" },
  { key: "Sat", label: "S" },
  { key: "Sun", label: "S" },
] as const;

const HOUR_INTERVALS = [1, 2, 3, 4, 6, 8, 12];

const MINUTES = [0, 15, 30, 45];

interface TimeEntry {
  hour: number;
  minute: number;
  ampm: "AM" | "PM";
}

interface ScheduleState {
  frequency: Frequency;
  selectedDays: string[];
  times: TimeEntry[];
  interval: number;
  dayOfMonth: number;
}

const DEFAULT_TIME: TimeEntry = { hour: 9, minute: 0, ampm: "AM" };

const DEFAULT_STATE: ScheduleState = {
  frequency: "weekly",
  selectedDays: ["Mon"],
  times: [{ ...DEFAULT_TIME }],
  interval: 1,
  dayOfMonth: 1,
};

function to24Hour(hour: number, ampm: "AM" | "PM"): number {
  if (ampm === "AM") return hour === 12 ? 0 : hour;
  return hour === 12 ? 12 : hour + 12;
}

function to12Hour(h24: number): { hour: number; ampm: "AM" | "PM" } {
  if (h24 === 0) return { hour: 12, ampm: "AM" };
  if (h24 < 12) return { hour: h24, ampm: "AM" };
  if (h24 === 12) return { hour: 12, ampm: "PM" };
  return { hour: h24 - 12, ampm: "PM" };
}

function cronFromState(s: ScheduleState): string {
  if (s.frequency === "hourly") {
    return `0 0 */${s.interval} * * * *`;
  }
  return s.times
    .map((t) => {
      const h = to24Hour(t.hour, t.ampm);
      switch (s.frequency) {
        case "daily":
          return `0 ${t.minute} ${h} * * * *`;
        case "weekdays":
          return `0 ${t.minute} ${h} * * Mon-Fri *`;
        case "weekly":
          return `0 ${t.minute} ${h} * * ${s.selectedDays.join(",")} *`;
        case "monthly":
          return `0 ${t.minute} ${h} ${s.dayOfMonth} * * *`;
      }
    })
    .join("; ");
}

const DAY_NAMES_LONG: Record<string, string> = {
  Mon: "Monday",
  Tue: "Tuesday",
  Wed: "Wednesday",
  Thu: "Thursday",
  Fri: "Friday",
  Sat: "Saturday",
  Sun: "Sunday",
};

const DAY_INDICES: Record<string, number> = {
  Sun: 0,
  Mon: 1,
  Tue: 2,
  Wed: 3,
  Thu: 4,
  Fri: 5,
  Sat: 6,
};

const NUM_TO_DAY: Record<number, string> = {
  0: "Sun",
  1: "Mon",
  2: "Tue",
  3: "Wed",
  4: "Thu",
  5: "Fri",
  6: "Sat",
  7: "Sun",
};

function normalizeCron(cron: string): string {
  const parts = cron.trim().split(/\s+/);
  if (parts.length === 5) {
    return `0 ${cron.trim()} *`;
  }
  if (parts.length === 6) {
    return `${cron.trim()} *`;
  }
  return cron;
}

function normalizeDow(dow: string): string {
  return dow.replace(/\b(\d)\b/g, (_, d) => NUM_TO_DAY[parseInt(d)] || d);
}

const LOCAL_TZ = Intl.DateTimeFormat().resolvedOptions().timeZone;

function convertCronFields(
  cronHour: number,
  cronMinute: number,
  fromTz: string,
): { hour: number; minute: number; dowShift: number } {
  if (fromTz === LOCAL_TZ) {
    return { hour: cronHour, minute: cronMinute, dowShift: 0 };
  }
  // Use a reference Monday (Jan 6, 2025 is a Monday) in UTC
  const ref =
    fromTz === "UTC"
      ? new Date(Date.UTC(2025, 0, 6, cronHour, cronMinute, 0))
      : (() => {
          // For arbitrary timezone: approximate using UTC offset difference
          // Create a date at the given hour in UTC, then find offset of fromTz
          const d = new Date(Date.UTC(2025, 0, 6, cronHour, cronMinute, 0));
          const utcStr = d.toLocaleString("en-US", {
            timeZone: "UTC",
            hour12: false,
          });
          const tzStr = d.toLocaleString("en-US", {
            timeZone: fromTz,
            hour12: false,
          });
          const utcH = parseInt(utcStr.split(", ")[1]?.split(":")[0] ?? "0");
          const tzH = parseInt(tzStr.split(", ")[1]?.split(":")[0] ?? "0");
          const offsetH = tzH - utcH;
          // The ref date is when it's cronHour in fromTz, so subtract that offset
          return new Date(
            Date.UTC(2025, 0, 6, cronHour - offsetH, cronMinute, 0),
          );
        })();
  const localDay = ref.getDay(); // 0=Sun..6=Sat
  const utcDay = ref.getUTCDay();
  const refDay = fromTz === "UTC" ? utcDay : 1; // Monday=1
  let dowShift = localDay - refDay;
  if (dowShift > 1) dowShift = -1; // wrapped around week boundary
  if (dowShift < -1) dowShift = 1;
  return { hour: ref.getHours(), minute: ref.getMinutes(), dowShift };
}

function shiftDayName(day: string, shift: number): string {
  if (shift === 0) return day;
  const ordered = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const idx = ordered.indexOf(day);
  if (idx < 0) return day;
  return ordered[(idx + shift + 7) % 7];
}

export function cronToHuman(cron: string, timezone?: string): string {
  const isMultiSchedule = cron.includes(";");
  const needsConvert = timezone && timezone !== LOCAL_TZ;
  const state = stateFromCron(cron, timezone);
  if (!state) {
    if (isMultiSchedule) {
      return cron
        .split(";")
        .map((part) => cronToHuman(part.trim(), timezone))
        .filter(Boolean)
        .join(" and ");
    }
    const normalized = normalizeCron(cron);
    const parts = normalized.trim().split(/\s+/);
    if (parts.length >= 6) {
      const [, min, hour, dom, , rawDow] = parts;
      const dow = normalizeDow(rawDow);
      const pieces: string[] = [];
      if (hour.startsWith("*/")) {
        pieces.push(`every ${hour.slice(2)}h`);
      } else if (hour !== "*") {
        let h = parseInt(hour);
        let m = parseInt(min) || 0;
        let dowShift = 0;
        if (needsConvert) {
          const conv = convertCronFields(h, m, timezone!);
          h = conv.hour;
          m = conv.minute;
          dowShift = conv.dowShift;
        }
        if (dow === "Mon-Fri") {
          pieces.push("Weekdays");
        } else if (dow !== "*") {
          const converted = dow
            .split(",")
            .map((d) => shiftDayName(d, dowShift));
          pieces.push(converted.map((d) => DAY_NAMES_LONG[d] || d).join(", "));
        }
        if (dom !== "*") pieces.push(`day ${dom}`);
        const { hour: h12, ampm } = to12Hour(h);
        pieces.push(`at ${h12}:${String(m).padStart(2, "0")} ${ampm}`);
      } else {
        if (dow === "Mon-Fri") pieces.push("Weekdays");
        else if (dow !== "*")
          pieces.push(
            dow
              .split(",")
              .map((d) => DAY_NAMES_LONG[d] || d)
              .join(", "),
          );
        if (dom !== "*") pieces.push(`day ${dom}`);
      }
      return pieces.join(" ") || cron;
    }
    return cron;
  }
  return humanFromState(state);
}

function formatTimeStr(t: TimeEntry): string {
  return `${t.hour}:${String(t.minute).padStart(2, "0")} ${t.ampm}`;
}

function formatTimeList(times: TimeEntry[]): string {
  const strs = times.map(formatTimeStr);
  if (strs.length === 1) return strs[0];
  if (strs.length === 2) return `${strs[0]} and ${strs[1]}`;
  const last = strs.pop()!;
  return `${strs.join(", ")}, and ${last}`;
}

function humanFromState(s: ScheduleState): string {
  const timeStr = formatTimeList(s.times);
  switch (s.frequency) {
    case "hourly":
      return s.interval === 1 ? "Every hour" : `Every ${s.interval} hours`;
    case "daily":
      return `Daily at ${timeStr}`;
    case "weekdays":
      return `Weekdays at ${timeStr}`;
    case "weekly": {
      const dayNames = [...s.selectedDays.map((d) => DAY_NAMES_LONG[d] || d)];
      let dayStr: string;
      if (dayNames.length === 1) dayStr = dayNames[0];
      else {
        const last = dayNames.pop()!;
        dayStr = `${dayNames.join(", ")} and ${last}`;
      }
      return `Every ${dayStr} at ${timeStr}`;
    }
    case "monthly":
      return `Monthly on the ${ordinal(s.dayOfMonth)} at ${timeStr}`;
  }
}

function ordinal(n: number): string {
  const s = ["th", "st", "nd", "rd"];
  const v = n % 100;
  return n + (s[(v - 20) % 10] || s[v] || s[0]);
}

function parseSingleCron(cron: string): ScheduleState | null {
  const normalized = normalizeCron(cron);
  const parts = normalized.trim().split(/\s+/);
  if (parts.length < 6 || parts.length > 7) return null;

  const [sec, min, hour, dom, , rawDow] = parts;
  if (sec !== "0") return null;
  const dow = normalizeDow(rawDow);

  const minVal = parseInt(min);
  const hourVal = parseInt(hour);

  if (hour.startsWith("*/")) {
    const interval = parseInt(hour.slice(2));
    if (HOUR_INTERVALS.includes(interval)) {
      return {
        ...DEFAULT_STATE,
        frequency: "hourly",
        interval,
        times: [{ ...DEFAULT_TIME }],
      };
    }
    return null;
  }

  if (isNaN(hourVal) || isNaN(minVal)) return null;
  const { hour: h12, ampm } = to12Hour(hourVal);
  const time: TimeEntry = { hour: h12, minute: minVal, ampm };

  if (dom !== "*") {
    const d = parseInt(dom);
    if (!isNaN(d) && d >= 1 && d <= 28 && dow === "*") {
      return {
        ...DEFAULT_STATE,
        frequency: "monthly",
        dayOfMonth: d,
        times: [time],
      };
    }
    return null;
  }

  if (dow === "*") {
    return { ...DEFAULT_STATE, frequency: "daily", times: [time] };
  }

  if (dow === "Mon-Fri") {
    return { ...DEFAULT_STATE, frequency: "weekdays", times: [time] };
  }

  const dayList = dow.split(",").filter((d) => d in DAY_NAMES_LONG);
  if (dayList.length > 0) {
    return {
      ...DEFAULT_STATE,
      frequency: "weekly",
      selectedDays: dayList,
      times: [time],
    };
  }

  return null;
}

function convertState(s: ScheduleState, fromTz: string): ScheduleState {
  if (!fromTz || fromTz === LOCAL_TZ) return s;
  if (s.frequency === "hourly") return s;
  const convertedTimes: TimeEntry[] = [];
  let dowShift = 0;
  for (const t of s.times) {
    const h24 = to24Hour(t.hour, t.ampm);
    const conv = convertCronFields(h24, t.minute, fromTz);
    const { hour: h12, ampm } = to12Hour(conv.hour);
    convertedTimes.push({ hour: h12, minute: conv.minute, ampm });
    dowShift = conv.dowShift;
  }
  let selectedDays = s.selectedDays;
  if (dowShift !== 0 && s.frequency === "weekly") {
    selectedDays = s.selectedDays.map((d) => shiftDayName(d, dowShift));
  }
  return { ...s, times: convertedTimes, selectedDays };
}

function stateFromCron(cron: string, timezone?: string): ScheduleState | null {
  if (cron.includes(";")) {
    const subExprs = cron
      .split(";")
      .map((s) => s.trim())
      .filter(Boolean);
    if (subExprs.length === 0) return null;
    const states = subExprs.map((s) => parseSingleCron(s));
    if (states.some((s) => s === null)) return null;
    const first = states[0]!;
    const allMatch = states.every(
      (s) =>
        s!.frequency === first.frequency &&
        JSON.stringify(s!.selectedDays) ===
          JSON.stringify(first.selectedDays) &&
        s!.dayOfMonth === first.dayOfMonth &&
        s!.interval === first.interval,
    );
    if (!allMatch) return null;
    const merged = { ...first, times: states.map((s) => s!.times[0]) };
    return timezone ? convertState(merged, timezone) : merged;
  }
  const result = parseSingleCron(cron);
  return result && timezone ? convertState(result, timezone) : result;
}

function getNextRun(s: ScheduleState): Date | null {
  const now = new Date();

  if (s.frequency === "hourly") {
    const next = new Date(now);
    next.setMinutes(0, 0, 0);
    next.setHours(
      next.getHours() + s.interval - (next.getHours() % s.interval),
    );
    if (next <= now) next.setHours(next.getHours() + s.interval);
    return next;
  }

  const targetDays =
    s.frequency === "daily"
      ? [0, 1, 2, 3, 4, 5, 6]
      : s.frequency === "weekdays"
        ? [1, 2, 3, 4, 5]
        : s.frequency === "monthly"
          ? null
          : s.selectedDays.map((d) => DAY_INDICES[d]).sort((a, b) => a - b);

  let earliest: Date | null = null;

  for (const t of s.times) {
    const h = to24Hour(t.hour, t.ampm);

    if (s.frequency === "monthly") {
      const next = new Date(
        now.getFullYear(),
        now.getMonth(),
        s.dayOfMonth,
        h,
        t.minute,
        0,
        0,
      );
      if (next <= now) next.setMonth(next.getMonth() + 1);
      if (!earliest || next < earliest) earliest = next;
      continue;
    }

    if (!targetDays || targetDays.length === 0) continue;

    for (let offset = 0; offset <= 7; offset++) {
      const candidate = new Date(now);
      candidate.setDate(candidate.getDate() + offset);
      candidate.setHours(h, t.minute, 0, 0);
      if (candidate <= now) continue;
      if (targetDays.includes(candidate.getDay())) {
        if (!earliest || candidate < earliest) earliest = candidate;
        break;
      }
    }
  }

  return earliest;
}

function formatNextRun(d: Date | null): string {
  if (!d) return "";
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const months = [
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
  ];
  const { hour, ampm } = to12Hour(d.getHours());
  return `${days[d.getDay()]}, ${months[d.getMonth()]} ${d.getDate()} at ${hour}:${String(d.getMinutes()).padStart(2, "0")} ${ampm}`;
}

function validateRawCron(cron: string): string | null {
  const segments = cron
    .split(";")
    .map((s) => s.trim())
    .filter(Boolean);
  if (segments.length === 0) return "Expression is empty";
  for (const seg of segments) {
    const parts = seg.split(/\s+/);
    if (parts.length < 5 || parts.length > 7) {
      return `Expected 5-7 fields, got ${parts.length} in "${seg}"`;
    }
    const validChars = /^[\d*/,a-zA-Z-]+$/;
    for (let i = 0; i < parts.length; i++) {
      if (!validChars.test(parts[i])) {
        return `Invalid characters in field ${i + 1}: "${parts[i]}"`;
      }
    }
  }
  return null;
}

export default function ScheduleBuilder({ value, onChange, timezone }: Props) {
  const builderId = useId();
  // Editing always preserves the workflow's source-timezone fields. Converting
  // the builder state to the viewer's timezone would silently rewrite the cron
  // expression when they switch modes or save without changing the schedule.
  const parsed = stateFromCron(value);
  const [isAdvanced, setIsAdvanced] = useState(!parsed && !!value);
  const [state, setState] = useState<ScheduleState>(parsed ?? DEFAULT_STATE);
  const [rawCron, setRawCron] = useState(value);
  const [rawError, setRawError] = useState<string | null>(null);

  const updateState = useCallback(
    (patch: Partial<ScheduleState>) => {
      setState((prev) => {
        const next = { ...prev, ...patch };
        onChange(cronFromState(next));
        return next;
      });
    },
    [onChange],
  );

  const updateTimeAt = useCallback(
    (idx: number, patch: Partial<TimeEntry>) => {
      setState((prev) => {
        const nextTimes = prev.times.map((t, i) =>
          i === idx ? { ...t, ...patch } : t,
        );
        const next = { ...prev, times: nextTimes };
        onChange(cronFromState(next));
        return next;
      });
    },
    [onChange],
  );

  const addTime = useCallback(() => {
    setState((prev) => {
      const next = { ...prev, times: [...prev.times, { ...DEFAULT_TIME }] };
      onChange(cronFromState(next));
      return next;
    });
  }, [onChange]);

  const removeTime = useCallback(
    (idx: number) => {
      setState((prev) => {
        if (prev.times.length <= 1) return prev;
        const next = { ...prev, times: prev.times.filter((_, i) => i !== idx) };
        onChange(cronFromState(next));
        return next;
      });
    },
    [onChange],
  );

  const handleAdvancedToggle = () => {
    if (isAdvanced) {
      const parsed = stateFromCron(rawCron);
      if (parsed) {
        setState(parsed);
        setIsAdvanced(false);
        setRawError(null);
      } else {
        setRawError(
          "Cannot parse this expression into the visual builder. Edit it here or clear to start fresh.",
        );
      }
    } else {
      setRawCron(cronFromState(state));
      setIsAdvanced(true);
      setRawError(null);
    }
  };

  const handleRawChange = (val: string) => {
    setRawCron(val);
    const err = validateRawCron(val);
    setRawError(err);
    if (!err) onChange(val);
  };

  const toggleDay = (day: string) => {
    const current = state.selectedDays;
    const next = current.includes(day)
      ? current.filter((d) => d !== day)
      : [...current, day];
    if (next.length === 0) return;
    const ordered = DAYS_OF_WEEK.map((d) => d.key).filter((k) =>
      next.includes(k),
    );
    updateState({ selectedDays: ordered });
  };

  const preview = isAdvanced ? cronToHuman(rawCron) : humanFromState(state);
  const nextRun = isAdvanced ? null : getNextRun(state);

  return (
    <div
      className="sched-builder"
      role="group"
      aria-labelledby={`${builderId}-label`}
    >
      <span className="editor-label" id={`${builderId}-label`}>
        Schedule
      </span>

      {!isAdvanced && (
        <>
          <div className="sched-freq-bar" role="group" aria-label="Frequency">
            {(
              [
                "hourly",
                "daily",
                "weekdays",
                "weekly",
                "monthly",
              ] as Frequency[]
            ).map((f) => (
              <button
                key={f}
                type="button"
                className={`sched-freq-btn ${state.frequency === f ? "active" : ""}`}
                aria-pressed={state.frequency === f}
                onClick={() => updateState({ frequency: f })}
              >
                {f.charAt(0).toUpperCase() + f.slice(1)}
              </button>
            ))}
          </div>

          <div className="sched-controls">
            {state.frequency === "hourly" && (
              <div className="sched-row">
                <span className="sched-label">Every</span>
                <Select
                  value={state.interval}
                  aria-label="Hour interval"
                  onChange={(e) =>
                    updateState({ interval: parseInt(e.target.value) })
                  }
                >
                  {HOUR_INTERVALS.map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </Select>
                <span className="sched-label">
                  {state.interval === 1 ? "hour" : "hours"}
                </span>
              </div>
            )}

            {state.frequency === "weekly" && (
              <div className="sched-days" role="group" aria-label="Days">
                {DAYS_OF_WEEK.map((d, i) => (
                  <button
                    key={d.key}
                    type="button"
                    className={`sched-day-pill ${state.selectedDays.includes(d.key) ? "active" : ""}`}
                    aria-label={DAY_NAMES_LONG[d.key]}
                    aria-pressed={state.selectedDays.includes(d.key)}
                    onClick={() => toggleDay(d.key)}
                    title={DAY_NAMES_LONG[d.key]}
                  >
                    <span className="sched-day-letter">{d.label}</span>
                    <span className="sched-day-abbr">
                      {["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][i]}
                    </span>
                  </button>
                ))}
              </div>
            )}

            {state.frequency === "monthly" && (
              <div className="sched-row">
                <span className="sched-label">On day</span>
                <Select
                  value={state.dayOfMonth}
                  aria-label="Day of month"
                  onChange={(e) =>
                    updateState({ dayOfMonth: parseInt(e.target.value) })
                  }
                >
                  {Array.from({ length: 28 }, (_, i) => i + 1).map((d) => (
                    <option key={d} value={d}>
                      {d}
                    </option>
                  ))}
                </Select>
              </div>
            )}

            {state.frequency !== "hourly" && (
              <div className="sched-times">
                {state.times.map((t, idx) => (
                  <div key={idx} className="sched-row sched-time-row">
                    <span className="sched-label">At</span>
                    <Select
                      value={t.hour}
                      aria-label={`Time ${idx + 1} hour`}
                      onChange={(e) =>
                        updateTimeAt(idx, { hour: parseInt(e.target.value) })
                      }
                    >
                      {Array.from({ length: 12 }, (_, i) => i + 1).map((h) => (
                        <option key={h} value={h}>
                          {h}
                        </option>
                      ))}
                    </Select>
                    <span className="sched-colon">:</span>
                    <Select
                      value={t.minute}
                      aria-label={`Time ${idx + 1} minute`}
                      onChange={(e) =>
                        updateTimeAt(idx, { minute: parseInt(e.target.value) })
                      }
                    >
                      {MINUTES.map((m) => (
                        <option key={m} value={m}>
                          {String(m).padStart(2, "0")}
                        </option>
                      ))}
                    </Select>
                    <div
                      className="sched-ampm-toggle"
                      role="group"
                      aria-label={`Time ${idx + 1} period`}
                    >
                      <button
                        type="button"
                        className={`sched-ampm-btn ${t.ampm === "AM" ? "active" : ""}`}
                        aria-pressed={t.ampm === "AM"}
                        onClick={() => updateTimeAt(idx, { ampm: "AM" })}
                      >
                        AM
                      </button>
                      <button
                        type="button"
                        className={`sched-ampm-btn ${t.ampm === "PM" ? "active" : ""}`}
                        aria-pressed={t.ampm === "PM"}
                        onClick={() => updateTimeAt(idx, { ampm: "PM" })}
                      >
                        PM
                      </button>
                    </div>
                    {state.times.length > 1 && (
                      <button
                        type="button"
                        className="sched-time-remove"
                        aria-label={`Remove time ${idx + 1}`}
                        onClick={() => removeTime(idx)}
                        title="Remove this time"
                      >
                        &times;
                      </button>
                    )}
                  </div>
                ))}
                <button
                  type="button"
                  className="sched-time-add"
                  onClick={addTime}
                >
                  + Add time
                </button>
              </div>
            )}
          </div>
        </>
      )}

      {isAdvanced && (
        <div className="sched-advanced">
          <input
            type="text"
            value={rawCron}
            aria-label="Cron expression"
            aria-invalid={rawError ? "true" : undefined}
            aria-describedby={`${builderId}-cron-hint${rawError ? ` ${builderId}-cron-error` : ""}`}
            onChange={(e) => handleRawChange(e.target.value)}
            placeholder="sec min hour day month weekday year"
            className={rawError ? "sched-input-error" : ""}
          />
          {rawError && (
            <span className="sched-error" id={`${builderId}-cron-error`}>
              {rawError}
            </span>
          )}
          <span className="editor-hint" id={`${builderId}-cron-hint`}>
            7-field cron: sec min hour day month weekday year. Use ; to separate
            multiple schedules.
            {timezone && timezone !== LOCAL_TZ
              ? ` Fields are in ${timezone}.`
              : " Fields are in local time."}
          </span>
        </div>
      )}

      <div className="sched-preview" aria-live="polite">
        <span className="sched-preview-text">{preview}</span>
        {nextRun && (
          <span className="sched-preview-next">
            Next run: {formatNextRun(nextRun)}
          </span>
        )}
      </div>

      <button
        type="button"
        className="sched-toggle-link"
        aria-expanded={isAdvanced}
        onClick={handleAdvancedToggle}
      >
        {isAdvanced ? "Use visual builder" : "Edit cron expression"}
      </button>
    </div>
  );
}
