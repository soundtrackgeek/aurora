import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ContentTransition } from "./ContentTransition";

afterEach(() => {
  cleanup();
});

vi.mock("react", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react")>();
  return {
    ...actual,
    ViewTransition: vi.fn(({ children, enter, update }: { children: React.ReactNode; enter?: unknown; update?: unknown }) => (
      <div data-testid="mock-view-transition" data-enter={JSON.stringify(enter)} data-update={JSON.stringify(update)}>
        {children}
      </div>
    )),
  };
});

describe("ContentTransition", () => {
  it("defaults update to none when enter is true and type is omitted", () => {
    const { getByTestId } = render(
      <ContentTransition enter>
        <div>Content</div>
      </ContentTransition>,
    );
    const element = getByTestId("mock-view-transition");
    expect(element.getAttribute("data-enter")).toBe(JSON.stringify("aurora-content"));
    expect(element.getAttribute("data-update")).toBe(JSON.stringify("none"));
  });

  it("configures type-specific transitions when type is provided", () => {
    const { getByTestId } = render(
      <ContentTransition type="album-detail" enter>
        <div>Content</div>
      </ContentTransition>,
    );
    const element = getByTestId("mock-view-transition");
    expect(element.getAttribute("data-enter")).toBe(JSON.stringify({ "album-detail": "aurora-content", default: "aurora-content" }));
    expect(element.getAttribute("data-update")).toBe(JSON.stringify({ "album-detail": "aurora-content", default: "none" }));
  });

  it("defaults update to aurora-content when enter is false and type is omitted", () => {
    const { getByTestId } = render(
      <ContentTransition>
        <div>Content</div>
      </ContentTransition>,
    );
    const element = getByTestId("mock-view-transition");
    expect(element.getAttribute("data-enter")).toBeNull();
    expect(element.getAttribute("data-update")).toBe(JSON.stringify("aurora-content"));
  });
});
