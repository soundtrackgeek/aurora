import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { chartPresets, emptyChartFilters, loadChartPage, loadChartQueue, monthPeriod, type ChartPageRequest } from "../../charts";
import { ChartPeriodControls } from "./ChartPeriodControls";

afterEach(cleanup);
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});

it("keeps filter edits private until Apply and discards Cancel", () => {
  const onFilters = vi.fn();
  render(<ChartPeriodControls period={chartPresets[0]} onApply={vi.fn()} onFilters={onFilters} />);
  fireEvent.click(screen.getByRole("button", { name: "Advanced filters" }));
  fireEvent.change(screen.getByLabelText("Country"), { target: { value: "NO" } });
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(onFilters).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Advanced filters" }));
  expect(screen.getByLabelText("Country")).toHaveValue("");
  fireEvent.change(screen.getByLabelText("Type"), { target: { value: "Group" } });
  expect(screen.queryByRole("option", { name: "Alive" })).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Status"), { target: { value: "disbanded" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply filters" }));
  expect(onFilters).toHaveBeenCalledWith({ country: "", artistType: "Group", status: "disbanded" });
});

it("converts a cross-year month range into inclusive chart weeks", () => {
  const onApply = vi.fn();
  render(<ChartPeriodControls period={chartPresets[0]} onApply={onApply} onFilters={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("Unit"), { target: { value: "month" } });
  fireEvent.change(screen.getByLabelText("From Month"), { target: { value: "12" } });
  fireEvent.change(screen.getByLabelText("To Year"), { target: { value: "1986" } });
  fireEvent.change(screen.getByLabelText("To Month"), { target: { value: "2" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply range" }));
  expect(onApply).toHaveBeenCalledWith({ fromYear: 1985, fromWeek: 48, toYear: 1986, toWeek: 9, label: "December 1985 – February 1986" });
  expect(monthPeriod(2021, 1, 2021, 12, "2021")).toMatchObject({ fromYear: 2021, fromWeek: 1, toYear: 2021, toWeek: 52 });
});

it("filters preview results and playback consistently before limiting", async () => {
  const request: ChartPageRequest = { kind: "singles", source: "officialUk", scope: "week", period: chartPresets[0], selectedYear: 1985, selectedWeek: 23, yearBasis: "year", limit: 1, filters: { ...emptyChartFilters, country: "GB", artistType: "Group" } };
  const page = await loadChartPage(request);
  expect(page.totalEntries).toBe(2);
  expect(page.entries.map((entry) => entry.artist)).toEqual(["Marillion"]);
  expect((await loadChartQueue(request)).map((track) => track.artist)).toEqual(["Marillion"]);
  const empty = await loadChartPage({ ...request, filters: { ...emptyChartFilters, country: "NO" } });
  expect(empty.entries).toEqual([]);
  expect(empty.totalEntries).toBe(0);
});
