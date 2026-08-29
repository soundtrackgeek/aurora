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

  it("uses country flags instead of abbreviations for album charts", () => {
    const ranks = [
      { source: "billboard", label: "Billboard", shortLabel: "US", rank: 1 },
      { source: "officialUk", label: "Official UK", shortLabel: "UK", rank: 2 },
      { source: "vgLista", label: "VG Lista", shortLabel: "NO", rank: 3 },
    ] as const;
    render(<CatalogChartRanks kind="album" ranks={ranks} />);

    const group = screen.getByLabelText("Chart rankings: Billboard number 1, Official UK number 2, VG Lista number 3");
    expect(screen.getByRole("img", { name: "United States chart" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "United Kingdom chart" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Norway chart" })).toBeInTheDocument();
    expect(group).toHaveTextContent("#1");
    expect(group).toHaveTextContent("#2");
    expect(group).toHaveTextContent("#3");
    expect(group).not.toHaveTextContent(/US|UK|NO/u);
  });
});
