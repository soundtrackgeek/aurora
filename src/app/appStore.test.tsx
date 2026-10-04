import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AppStoreProvider } from "./AppStoreProvider";
import { useAppSlice } from "./useAppStore";
import { createAppStore } from "./appStore";

afterEach(cleanup);

it("only renders consumers whose shared slice changed", () => {
  const store = createAppStore("Albums");
  const renderedDestination = vi.fn();
  function Destination() {
    const destination = useAppSlice("activeDestination");
    renderedDestination(destination);
    return <span>{destination}</span>;
  }
  function Revision() { return <output>{useAppSlice("catalogRevision")}</output>; }
  render(<AppStoreProvider store={store}><Destination /><Revision /></AppStoreProvider>);
  act(() => store.setCatalogRevision((previous) => previous + 1));
  expect(screen.getByRole("status")).toHaveTextContent("1");
  expect(renderedDestination).toHaveBeenCalledTimes(1);
  act(() => store.setActiveDestination("Charts"));
  expect(screen.getByText("Charts")).toBeInTheDocument();
  expect(renderedDestination).toHaveBeenCalledTimes(2);
});

it("isolates app instances and ignores unchanged writes", () => {
  const first = createAppStore("Songs");
  const second = createAppStore("Albums");
  const listener = vi.fn();
  const unsubscribe = first.subscribe(listener);
  first.setActiveDestination("Songs");
  expect(listener).not.toHaveBeenCalled();
  first.setActiveDestination("Charts");
  expect(listener).toHaveBeenCalledTimes(1);
  expect(second.getSnapshot().activeDestination).toBe("Albums");
  unsubscribe();
  first.setCatalogRevision(2);
  expect(listener).toHaveBeenCalledTimes(1);
});
