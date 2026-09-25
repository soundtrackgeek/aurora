import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { JevSettingsPanel } from "./JevSettingsPanel";
import { loadJevSettings, saveJevCredentials, testJevConnection } from "../tonight";
vi.mock("../tonight", () => ({ loadJevSettings: vi.fn(), saveJevCredentials: vi.fn(), testJevConnection: vi.fn() }));
beforeEach(() => { vi.clearAllMocks(); vi.mocked(loadJevSettings).mockResolvedValue({ configured: false, source: "none", model: "typesafe/jev-1.13" }); });
afterEach(cleanup);
it("clears the draft key after vault save and tests only explicitly", async () => {
  vi.mocked(saveJevCredentials).mockResolvedValue({ configured: true, source: "vault", model: "typesafe/jev-1.13" });
  vi.mocked(testJevConnection).mockResolvedValue("Typed score received.");
  render(<JevSettingsPanel />); await waitFor(() => expect(loadJevSettings).toHaveBeenCalledOnce());
  fireEvent.change(screen.getByLabelText("OpenRouter API key"), { target: { value: "test-only-key" } });
  fireEvent.click(screen.getByRole("button", { name: "Save OpenRouter key" }));
  await screen.findByText("OpenRouter key saved securely.");
  expect(screen.getByLabelText("OpenRouter API key")).toHaveValue("");
  expect(testJevConnection).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Test Jev connection" }));
  expect(await screen.findByText("Typed score received.")).toBeVisible();
});
it("does not accept or persist keys in browser preview", async () => {
  vi.mocked(loadJevSettings).mockResolvedValue({ configured: false, source: "preview", model: "typesafe/jev-1.13" });
  render(<JevSettingsPanel />);
  await screen.findByText(/This browser preview never stores API keys/);
  expect(screen.getByLabelText("OpenRouter API key")).toBeDisabled();
  expect(screen.getByRole("button", { name: "Test Jev connection" })).toBeDisabled();
});
