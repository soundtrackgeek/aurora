import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defaultShortcutBindings, type GlobalShortcutStatus } from "../shortcuts";
import { acceleratorFromEvent } from "../shortcutCapture";
import { SettingsDialog } from "./SettingsDialog";

const status: GlobalShortcutStatus = {
  enabled: true,
  registered: true,
  platformAvailable: true,
  error: null,
  warning: null,
  bindings: defaultShortcutBindings,
};

afterEach(cleanup);

describe("SettingsDialog", () => {
  it("shows the requested defaults and guarantees the now-playing scope", () => {
    render(<SettingsDialog status={status} saving={false} error={null} onSave={() => undefined} onClose={() => undefined} />);

    expect(screen.getByRole("dialog", { name: "Global shortcuts" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Change Play or pause shortcut.*Ctrl\+Alt\+P/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Change Clear rating shortcut.*Ctrl\+Alt\+0/ })).toBeInTheDocument();
    expect(screen.getByText("Now playing is the only target.")).toBeInTheDocument();
    expect(screen.getByText(/Selecting another song in Explore never changes/)).toBeInTheDocument();
  });

  it("records custom bindings and saves the complete set", () => {
    const onSave = vi.fn();
    render(<SettingsDialog status={status} saving={false} error={null} onSave={onSave} onClose={() => undefined} />);

    fireEvent.click(screen.getByRole("button", { name: /Change Play or pause shortcut/ }));
    fireEvent.keyDown(window, { code: "KeyK", key: "k", ctrlKey: true, shiftKey: true });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSave).toHaveBeenCalledOnce();
    expect(onSave.mock.calls[0][0].bindings).toHaveLength(9);
    expect(onSave.mock.calls[0][0].bindings[0]).toEqual({ action: "playPause", accelerator: "Ctrl+Shift+K" });
  });

  it("rejects duplicate shortcuts before native registration", () => {
    render(<SettingsDialog status={status} saving={false} error={null} onSave={() => undefined} onClose={() => undefined} />);

    fireEvent.click(screen.getByRole("button", { name: /Change Play or pause shortcut/ }));
    fireEvent.keyDown(window, { code: "KeyN", key: "n", ctrlKey: true, altKey: true });

    expect(screen.getByRole("alert")).toHaveTextContent("Ctrl+Alt+N is assigned to both Play or pause and Next track");
    expect(screen.getByRole("button", { name: "Save changes" })).toBeDisabled();
  });

  it("restores every default binding", () => {
    render(<SettingsDialog
      status={{
        ...status,
        bindings: status.bindings.map((item, index) => index === 0 ? { ...item, accelerator: "Ctrl+Shift+K" } : item),
      }}
      saving={false}
      error={null}
      onSave={() => undefined}
      onClose={() => undefined}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Restore defaults" }));
    expect(screen.getByRole("button", { name: /Change Play or pause shortcut.*Ctrl\+Alt\+P/ })).toBeInTheDocument();
  });
});

describe("acceleratorFromEvent", () => {
  it("requires a modifier and preserves numeric keypad identity", () => {
    expect(acceleratorFromEvent(new KeyboardEvent("keydown", { code: "KeyP" }))).toBeNull();
    expect(acceleratorFromEvent(new KeyboardEvent("keydown", {
      code: "Numpad3",
      ctrlKey: true,
      altKey: true,
    }))).toBe("Ctrl+Alt+Numpad3");
  });
});
