import { beforeEach, describe, expect, it } from "vitest";
import {
  loadAudioSettings,
  resetAudioPreview,
  updateAudioSettings,
} from "./audio";

beforeEach(resetAudioPreview);

describe("browser audio settings adapter", () => {
  it("starts with safe device-local defaults", async () => {
    const status = await loadAudioSettings();
    expect(status.settings).toEqual({
      outputDeviceId: "system-default",
      replayGainMode: "off",
    });
    expect(status.activeDeviceLabel).toBe("Speakers (Realtek Audio)");
  });

  it("persists the selected output and ReplayGain mode in preview state", async () => {
    const status = await updateAudioSettings({
      outputDeviceId: "preview-dac",
      replayGainMode: "album",
    });
    expect(status.settings.replayGainMode).toBe("album");
    expect(status.activeDeviceLabel).toBe("USB DAC");
    expect((await loadAudioSettings()).settings.outputDeviceId).toBe("preview-dac");
  });
});
