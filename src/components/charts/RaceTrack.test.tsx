import { afterEach, describe, it, expect } from "vitest";
import { cleanup, render } from "@testing-library/react";
import RaceTrack from "./RaceTrack";
import type { RaceTrackJob } from "./RaceTrack";

afterEach(cleanup);

const JOBS: RaceTrackJob[] = [
  { job: "ingest-events", elapsedSeconds: 660, expectedSeconds: 780 },
  { job: "nightly-etl", elapsedSeconds: 360, expectedSeconds: 720 },
];

function vehicleStyles(container: HTMLElement): (string | null)[] {
  return Array.from(container.querySelectorAll("[data-vehicle-style]")).map(
    (el) => el.getAttribute("data-vehicle-style"),
  );
}

describe("RaceTrack", () => {
  it("renders an accessible summary and one lane per job", () => {
    const { getByRole, getAllByRole, container } = render(
      <RaceTrack jobs={[{ ...JOBS[0], agent: "worker-east-01" }, JOBS[1]]} />,
    );
    const svg = getByRole("img");
    expect(svg.getAttribute("aria-label")).toContain("ingest-events");
    expect(svg.getAttribute("aria-label")).toContain("on worker-east-01");
    expect(container.querySelectorAll("[data-vehicle-style]")).toHaveLength(2);
    // The lane cars are decorative — only the root svg is exposed as an image.
    expect(getAllByRole("img")).toHaveLength(1);
  });

  it("maps expected runtime to a vehicle class (quick → sedan, long → truck)", () => {
    const { container } = render(
      <RaceTrack
        jobs={[
          { job: "a", elapsedSeconds: 60, expectedSeconds: 600 }, // 10m
          { job: "b", elapsedSeconds: 60, expectedSeconds: 1200 }, // 20m
          { job: "c", elapsedSeconds: 60, expectedSeconds: 2100 }, // 35m
          { job: "d", elapsedSeconds: 60, expectedSeconds: 3000 }, // 50m
        ]}
      />,
    );
    expect(vehicleStyles(container)).toEqual([
      "sedan",
      "coupe",
      "racer",
      "truck",
    ]);
  });

  it("flags an overrunning job red on the car and its sub-label", () => {
    const { container } = render(
      <RaceTrack
        jobs={[
          { job: "risk-scoring", elapsedSeconds: 2040, expectedSeconds: 1800 },
        ]}
      />,
    );
    const car = container.querySelector("[data-vehicle-style]");
    expect(car?.hasAttribute("data-vehicle-over")).toBe(true);
    expect(car?.querySelector("path, rect")?.getAttribute("style")).toContain(
      "var(--error)",
    );
    expect(container.querySelector(".cs-racetrack__sub--over")).not.toBeNull();
  });

  it("keeps cars in their lane color and off-red while on pace", () => {
    const { container } = render(<RaceTrack jobs={JOBS} />);
    const car = container.querySelector("[data-vehicle-style]");
    expect(car?.hasAttribute("data-vehicle-over")).toBe(false);
    expect(container.querySelector(".cs-racetrack__sub--over")).toBeNull();
  });

  it("formats durations with the shared duration util", () => {
    const { container } = render(
      <RaceTrack
        jobs={[
          { job: "nightly-etl", elapsedSeconds: 360, expectedSeconds: 720 },
        ]}
      />,
    );
    expect(container.querySelector(".cs-racetrack__sub")?.textContent).toBe(
      "6m 0s / ~12m 0s",
    );
  });

  it("renders an empty state with no jobs", () => {
    const { getByText, getByRole, container } = render(<RaceTrack jobs={[]} />);
    expect(getByText("No running jobs")).toBeInTheDocument();
    expect(getByRole("img").getAttribute("aria-label")).toBe("No running jobs");
    expect(container.querySelectorAll("[data-vehicle-style]")).toHaveLength(0);
  });

  it("only animates when opted in (deterministic by default)", () => {
    const { container, rerender } = render(<RaceTrack jobs={JOBS} />);
    expect(container.querySelector(".cs-racetrack--animated")).toBeNull();
    rerender(<RaceTrack jobs={JOBS} animate />);
    expect(container.querySelector(".cs-racetrack--animated")).not.toBeNull();
  });

  it("shows the default title, a custom title, or none", () => {
    const { getByText, queryByText, rerender, container } = render(
      <RaceTrack jobs={JOBS} />,
    );
    expect(getByText(/Running — vehicle lanes/)).toBeInTheDocument();
    rerender(<RaceTrack jobs={JOBS} title="Race" />);
    expect(getByText("Race")).toBeInTheDocument();
    rerender(<RaceTrack jobs={JOBS} title={null} />);
    expect(queryByText(/Running — vehicle lanes/)).toBeNull();
    expect(container.querySelector(".cs-racetrack__title")).toBeNull();
  });
});
