import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import App from "./App";
import * as charts from "./charts";
import * as library from "./library";

vi.mock("./components/WaveformTimeline", () => ({ WaveformTimeline: () => null }));

afterEach(() => { cleanup(); vi.restoreAllMocks(); localStorage.clear(); });

async function navigate(name: string) {
  const primary = await screen.findByRole("navigation", { name: "Primary" });
  fireEvent.click(within(primary).getByRole("button", { name }));
}

it.each([
  { method: "back", setting: "source", tab: "VG Lista", title: "VG Lista Singles Chart" },
  { method: "sidebar", setting: "source", tab: "VG Lista", title: "VG Lista Singles Chart" },
  { method: "back", setting: "period", tab: "Period chart", title: "Official UK Singles · Summer 1985" },
  { method: "sidebar", setting: "period", tab: "Period chart", title: "Official UK Singles · Summer 1985" },
])("retains Charts $setting, selected row and scroll through $method navigation", async ({ method, tab, title }) => {
  const load = vi.spyOn(charts, "loadChartPage");
  const { container } = render(<App />);
  const primary = within(await screen.findByRole("navigation", { name: "Primary" }));
  const main = within(screen.getByRole("main"));
  fireEvent.click(primary.getByRole("button", { name: "Charts" }));
  const heading = await main.findByRole("heading", { name: "Official UK Singles Chart" });
  const studio = within(heading.closest(".chart-studio") as HTMLElement);
  fireEvent.click(studio.getByRole("tab", { name: tab }));
  const table = within(await studio.findByRole("table", { name: title }));
  fireEvent.click(table.getByRole("row", { name: /Obsession/ }));
  const scroll = container.querySelector<HTMLElement>(".main-scroll")!;
  scroll.scrollTop = 640;
  fireEvent.scroll(scroll);
  const requests = load.mock.calls.length;
  fireEvent.click(primary.getByRole("button", { name: "Years" }));
  await main.findByRole("tab", { name: "Two clocks" });
  expect(studio.queryByRole("table", { name: title })).not.toBeInTheDocument();
  if (method === "back") fireEvent.click(main.getByRole("button", { name: "Back to Charts" }));
  else fireEvent.click(primary.getByRole("button", { name: "Charts" }));
  // Re-query the returned page so an obsolete, detached DOM node cannot pass the test.
  const returnedHeading = await main.findByRole("heading", { name: title });
  expect(returnedHeading).toBeVisible();
  const returnedStudio = within(returnedHeading.closest(".chart-studio") as HTMLElement);
  expect(returnedStudio.getByRole("tab", { name: tab })).toHaveAttribute("aria-selected", "true");
  const returnedTable = within(returnedStudio.getByRole("table", { name: title }));
  expect(returnedTable.getByRole("row", { name: /Obsession/ })).toHaveAttribute("aria-selected", "true");
  await waitFor(() => expect(scroll.scrollTop).toBe(640));
  expect(load).toHaveBeenCalledTimes(requests);
});

it("keeps Albums filters and expanded detail separate from a Songs search", async () => {
  const load = vi.spyOn(library, "exploreAlbums");
  render(<App />);
  await navigate("Albums");
  fireEvent.click(await screen.findByRole("button", { name: /^Viva la Vida cover/ }));
  await screen.findByRole("complementary", { name: "Viva la Vida album details" });
  const requests = load.mock.calls.length;
  await navigate("Songs");
  fireEvent.change(screen.getByRole("textbox", { name: "Search your music universe" }), { target: { value: "Midnight" } });
  await waitFor(() => expect(screen.getByRole("textbox", { name: "Search your music universe" })).toHaveValue("Midnight"));
  await navigate("Albums");
  expect(await screen.findByRole("complementary", { name: "Viva la Vida album details" })).toBeVisible();
  expect(screen.getByRole("textbox", { name: "Search your music universe" })).toHaveValue("");
  expect(load).toHaveBeenCalledTimes(requests);
  fireEvent.click(screen.getByRole("button", { name: "Back to Songs" }));
  expect(screen.getByRole("textbox", { name: "Search your music universe" })).toHaveValue("Midnight");
  fireEvent.click(screen.getByRole("button", { name: "Back to Albums" }));
  expect(await screen.findByRole("complementary", { name: "Viva la Vida album details" })).toBeVisible();
});

it.each([
  ["Years", "Original landscape"],
  ["Ratings", "Album ratings"],
])("retains the selected %s view", async (page, tab) => {
  render(<App />);
  await navigate(page);
  fireEvent.click(await screen.findByRole("tab", { name: tab }));
  expect(screen.getByRole("tab", { name: tab })).toHaveAttribute("aria-selected", "true");
  await navigate("Charts");
  await screen.findByRole("heading", { name: "Official UK Singles Chart" });
  fireEvent.click(screen.getByRole("button", { name: `Back to ${page}` }));
  expect(await screen.findByRole("tab", { name: tab })).toHaveAttribute("aria-selected", "true");
});

it("returns to the History subpage rather than resetting to the report", async () => {
  render(<App />);
  await navigate("History");
  const historyNavigation = await screen.findByRole("navigation", { name: "Listening memory pages" });
  fireEvent.click(within(historyNavigation).getByRole("button", { name: "History" }));
  await screen.findByRole("heading", { name: "Your music remembers." });
  await navigate("Universe");
  await screen.findByRole("region", { name: "Library overview" });
  fireEvent.click(screen.getByRole("button", { name: "Back to History" }));
  expect(await screen.findByRole("heading", { name: "Your music remembers." })).toBeVisible();
});

it("retraces Artist to album drilldowns with the Artist tab preserved", async () => {
  render(<App />);
  await navigate("Albums");
  fireEvent.click(await screen.findByRole("button", { name: /^Viva la Vida cover/ }));
  const details = await screen.findByRole("complementary", { name: "Viva la Vida album details" });
  fireEvent.click(within(details).getByRole("button", { name: "Open artist page for Coldplay" }));
  const artist = await screen.findByRole("article", { name: "Coldplay artist page" });
  fireEvent.click(within(artist).getByRole("button", { name: "Albums" }));
  fireEvent.click(await screen.findByRole("button", { name: "Open Viva la Vida" }));
  await screen.findByRole("complementary", { name: "Viva la Vida album details" });
  fireEvent.click(screen.getByRole("button", { name: "Back to Coldplay" }));
  const returned = await screen.findByRole("article", { name: "Coldplay artist page" });
  expect(within(returned).getByRole("button", { name: "Albums" })).toHaveAttribute("aria-current", "page");
  fireEvent.click(screen.getByRole("button", { name: "Back to Albums" }));
  expect(await screen.findByRole("complementary", { name: "Viva la Vida album details" })).toBeVisible();
});

it.each(["Inbox", "Observatory", "Songs", "Albums", "Artists", "Publishers", "Genres", "Years", "Ratings", "Tags", "Charts", "History"])("provides a working Back action on %s", async (page) => {
  render(<App />);
  await screen.findByRole("region", { name: "Library overview" });
  expect(screen.getByRole("button", { name: "Back" })).toBeDisabled();
  await navigate(page);
  fireEvent.click(await screen.findByRole("button", { name: "Back to Universe" }));
  expect(await screen.findByRole("region", { name: "Library overview" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Back" })).toBeDisabled();
});

