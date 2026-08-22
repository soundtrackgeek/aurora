import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { YearsPlaceholder } from "./YearsPlaceholder";

afterEach(cleanup);

describe("YearsPlaceholder", () => {
  it("labels the route as future work without presenting fabricated year data", () => {
    render(<YearsPlaceholder />);

    expect(screen.getByRole("heading", { name: "Your collection through time." })).toBeInTheDocument();
    expect(screen.getByText("Placeholder in Aurora 0.12.0")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});
