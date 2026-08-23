import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AudioSettingsStatus } from "../audio";
import { defaultShortcutBindings, type GlobalShortcutStatus } from "../shortcuts";
import { acceleratorFromEvent } from "../shortcutCapture";
import { createDefaultDisplayPreferences } from "../displayPreferences";
import { SettingsDialog } from "./SettingsDialog";

const shortcutStatus: GlobalShortcutStatus = {
  enabled: true,
  registered: true,
  platformAvailable: true,
  error: null,
  warning: null,
  bindings: defaultShortcutBindings,
};

const audioStatus: AudioSettingsStatus = {
  settings: { outputDeviceId: "system-default", replayGainMode: "off" },
  devices: [
    { id: "speakers", label: "Speakers (Realtek Audio)", isDefault: true },
    { id: "dac", label: "USB DAC", isDefault: false },
  ],
  activeDeviceId: "speakers",
  activeDeviceLabel: "Speakers (Realtek Audio)",
  usingFallback: false,
  message: null,
  error: null,
};

afterEach(cleanup);

function renderSettings(overrides: Partial<Parameters<typeof SettingsDialog>[0]> = {}) {
  const props: Parameters<typeof SettingsDialog>[0] = {
    shortcutStatus,
    audioStatus,
    shortcutSaving: false,
    audioSaving: false,
    shortcutError: null,
    audioError: null,
    displayPreferences: createDefaultDisplayPreferences(),
    activeDisplayView: "charts",
    initialTab: "shortcuts",
    onSaveDisplay: vi.fn(),
    onSaveShortcuts: vi.fn(),
    onSaveAudio: vi.fn(),
    onClose: vi.fn(),
    ...overrides,
  };
  return { ...render(<SettingsDialog {...props} />), props };
}

describe("SettingsDialog", () => {
  it("shows the requested shortcut defaults and guarantees the now-playing scope", () => {
    renderSettings();

    expect(screen.getByRole("dialog", { name: "Global shortcuts" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Change Play or pause shortcut.*Ctrl\+Alt\+P/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Change Clear rating shortcut.*Ctrl\+Alt\+0/ })).toBeInTheDocument();
    expect(screen.getByText("Now playing is the only target.")).toBeInTheDocument();
  });

  it("records custom bindings and saves the complete set", () => {
    const onSaveShortcuts = vi.fn();
    renderSettings({ onSaveShortcuts });

    fireEvent.click(screen.getByRole("button", { name: /Change Play or pause shortcut/ }));
    fireEvent.keyDown(window, { code: "KeyK", key: "k", ctrlKey: true, shiftKey: true });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSaveShortcuts).toHaveBeenCalledOnce();
    expect(onSaveShortcuts.mock.calls[0][0].bindings).toHaveLength(9);
    expect(onSaveShortcuts.mock.calls[0][0].bindings[0]).toEqual({ action: "playPause", accelerator: "Ctrl+Shift+K" });
  });

  it("rejects duplicate shortcuts before native registration", () => {
    renderSettings();

    fireEvent.click(screen.getByRole("button", { name: /Change Play or pause shortcut/ }));
    fireEvent.keyDown(window, { code: "KeyN", key: "n", ctrlKey: true, altKey: true });

    expect(screen.getByRole("alert")).toHaveTextContent("Ctrl+Alt+N is assigned to both Play or pause and Next track");
    expect(screen.getByRole("button", { name: "Save changes" })).toBeDisabled();
  });

  it("restores every default binding", () => {
    renderSettings({
      shortcutStatus: {
        ...shortcutStatus,
        bindings: shortcutStatus.bindings.map((item, index) => index === 0 ? { ...item, accelerator: "Ctrl+Shift+K" } : item),
      },
    });

    fireEvent.click(screen.getByRole("button", { name: "Restore defaults" }));
    expect(screen.getByRole("button", { name: /Change Play or pause shortcut.*Ctrl\+Alt\+P/ })).toBeInTheDocument();
  });

  it("selects an output and ReplayGain mode as one device-local audio change", () => {
    const onSaveAudio = vi.fn();
    renderSettings({ initialTab: "audio", onSaveAudio });

    fireEvent.change(screen.getByRole("combobox"), { target: { value: "dac" } });
    fireEvent.click(screen.getByRole("radio", { name: /Album/ }));
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSaveAudio).toHaveBeenCalledWith({ outputDeviceId: "dac", replayGainMode: "album" });
    expect(screen.getByText("Clipping protection is always on")).toBeInTheDocument();
    expect(screen.getByText("Gapless queue transitions")).toBeInTheDocument();
  });

  it("saves global sizing and independent per-view overrides", () => {
    const onSaveDisplay = vi.fn();
    renderSettings({ initialTab: "display", onSaveDisplay });

    fireEvent.change(screen.getByRole("combobox", { name: "Global text size" }), { target: { value: "large" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Global cover size" }), { target: { value: "large" } });
    fireEvent.change(screen.getByRole("combobox", { name: "View to customize" }), { target: { value: "albums" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Albums text size" }), { target: { value: "maximum" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Albums cover size" }), { target: { value: "extra-large" } });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSaveDisplay).toHaveBeenCalledWith(expect.objectContaining({
      global: { textSize: "large", coverSize: "large" },
      views: expect.objectContaining({
        albums: { textSize: "maximum", coverSize: "extra-large" },
      }),
    }));
  });

  it("disables cover overrides for views without adjustable artwork", () => {
    renderSettings({ initialTab: "display", activeDisplayView: "observatory" });

    expect(screen.getByRole("combobox", { name: "Observatory cover size" })).toBeDisabled();
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
