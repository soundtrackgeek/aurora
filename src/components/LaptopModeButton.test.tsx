import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { LaptopModeStatus } from "../laptopMode";
import { LaptopModeButton } from "./LaptopModeButton";

const status: LaptopModeStatus = {
  laptopMode: false,
  modeLabel: "Desktop Mode",
  syncState: "synced",
  message: "Aurora state matches the verified OneDrive snapshot.",
  remotePath: "C:\\Users\\Jorn\\OneDrive\\_musicbackup\\aurora-state.sqlite3",
  lastSyncedAtMs: 1,
  settingWarning: null,
  mappings: [
    { desktopRoot: "D:\\MUSIC", laptopRoot: "Y:\\MUSIC", activeRoot: "D:\\MUSIC", available: true },
  ],
};

describe("LaptopModeButton", () => {
  it("exposes an icon-only accessible toggle and the exact mapping", () => {
    const onToggle = vi.fn();
    render(<LaptopModeButton status={status} busy={false} error={null} onToggle={onToggle} />);

    const button = screen.getByRole("button", { name: /Enable Laptop Mode/ });
    expect(button).toHaveAttribute("aria-pressed", "false");
    expect(screen.getAllByText("D:\\MUSIC")).toHaveLength(2);
    fireEvent.click(button);
    expect(onToggle).toHaveBeenCalledOnce();
  });

  it("surfaces conflicts without pretending the mirror is synchronized", () => {
    render(
      <LaptopModeButton
        status={{ ...status, syncState: "conflict", message: "Both computers changed." }}
        busy={false}
        error={null}
        onToggle={() => undefined}
      />,
    );
    expect(screen.getByText("Both computers changed.")).toBeInTheDocument();
    expect(document.querySelector(".laptop-mode-control.is-conflict")).toBeInTheDocument();
  });
});
