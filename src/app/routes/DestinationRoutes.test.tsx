import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { RememberedPage } from "../../components/navigation/RememberedPage";
import { AppStoreProvider } from "../AppStoreProvider";
import { createAppStore } from "../appStore";
import { ChartsRoute } from "./DestinationRoutes";

const evidence = vi.hoisted(() => ({ moduleLoads: 0, renders: 0 }));

vi.mock("../../components/charts/ChartStudio", () => {
  evidence.moduleLoads += 1;
  return {
    ChartStudio: function ChartStudio({ catalogRevision }: { catalogRevision: number; }) {
      const [query, setQuery] = useState("");
      evidence.renders += 1;
      return <section aria-label="Chart route fixture">
        <input aria-label="Chart search" value={query} onChange={(event) => setQuery(event.target.value)} />
        <output aria-label="Catalog revision">{catalogRevision}</output>
      </section>;
    },
    ChartInspector: () => null,
  };
});

afterEach(cleanup);

it("loads a destination on its first visit, retains its state, and subscribes only to its catalog slice", async () => {
  const store = createAppStore("Universe");
  const props = {
    onSelectionChange: vi.fn(),
    onSelectTrack: vi.fn(),
    onPlayQueue: vi.fn().mockResolvedValue(true),
    onOpenArtistAlbums: vi.fn(),
  };
  function Harness({ active }: { active: boolean; }) {
    return <AppStoreProvider store={store}>
      <RememberedPage active={active}><ChartsRoute {...props} /></RememberedPage>
    </AppStoreProvider>;
  }

  const view = render(<Harness active={false} />);
  expect(evidence.moduleLoads).toBe(0);
  expect(screen.queryByRole("region", { name: "Chart route fixture" })).not.toBeInTheDocument();

  view.rerender(<Harness active />);
  expect(screen.getByRole("status")).toHaveAttribute("data-route-loading");
  const search = await screen.findByRole("textbox", { name: "Chart search" });
  expect(evidence.moduleLoads).toBe(1);
  fireEvent.change(search, { target: { value: "retained chart search" } });
  const previousRenders = evidence.renders;
  act(() => store.setActiveDestination("Charts"));
  expect(evidence.renders).toBe(previousRenders);
  act(() => store.setCatalogRevision(7));
  expect(screen.getByLabelText("Catalog revision")).toHaveTextContent("7");

  view.rerender(<Harness active={false} />);
  expect(screen.queryByRole("textbox", { name: "Chart search" })).not.toBeInTheDocument();
  act(() => store.setCatalogRevision(8));
  view.rerender(<Harness active />);
  await waitFor(() => expect(screen.getByRole("textbox", { name: "Chart search" })).toHaveValue("retained chart search"));
  expect(screen.getByLabelText("Catalog revision")).toHaveTextContent("8");
  expect(evidence.moduleLoads).toBe(1);
});
