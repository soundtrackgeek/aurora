import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { CatalogChartRanks } from "./CatalogChartRanks";

describe("CatalogChartRanks", () => {
  it("renders Music Library's canonical rank once per chart", () => {
    render(<CatalogChartRanks kind="track" ranks={[
      { source: "billboard", label: "Billboard", shortLabel: "BB", rank: 4 },
      { source: "officialUk", label: "Official UK", shortLabel: "UK", rank: 14 },
    ]} />);
    expect(screen.getByLabelText("Chart rankings: Billboard number 4, Official UK number 14")).toHaveTextContent("BB:#4");
    expect(screen.getByLabelText("Chart rankings: Billboard number 4, Official UK number 14")).toHaveTextContent("UK:#14");
  });

  it("omits the chart group when the item has no materialized ranks", () => {
    const { container } = render(<CatalogChartRanks kind="album" ranks={[]} />);
    expect(container).toBeEmptyDOMElement();
  });
});
