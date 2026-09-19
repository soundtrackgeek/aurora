import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import * as data from "../artistPage";
import { FanartSettingsPanel } from "./FanartSettingsPanel";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });
it("saves project and personal keys through the vault command and clears the input fields", async () => {
  vi.spyOn(data, "loadFanartSettings").mockResolvedValue({ configured: false, personalKeyConfigured: false });
  const save = vi.spyOn(data, "saveFanartCredentials").mockResolvedValue({ configured: true, personalKeyConfigured: true });
  render(<FanartSettingsPanel />);
  const project = screen.getByLabelText("fanart.tv project API key");
  const personal = screen.getByLabelText("Personal / VIP key (optional)");
  expect(project).toHaveAttribute("type", "password");
  expect(personal).toHaveAttribute("type", "password");
  fireEvent.change(project, { target: { value: "test-project-key" } });
  fireEvent.change(personal, { target: { value: "test-personal-key" } });
  fireEvent.click(screen.getByRole("button", { name: "Save fanart.tv credentials" }));
  await waitFor(() => expect(save).toHaveBeenCalledWith({ mode: "save", apiKey: "test-project-key", personalKey: "test-personal-key" }));
  expect(await screen.findByText("Connected · personal key")).toBeInTheDocument();
  expect(project).toHaveValue("");
  expect(personal).toHaveValue("");
});

it("removes saved credentials without requiring the original keys", async () => {
  vi.spyOn(data, "loadFanartSettings").mockResolvedValue({ configured: true, personalKeyConfigured: false });
  const save = vi.spyOn(data, "saveFanartCredentials").mockResolvedValue({ configured: false, personalKeyConfigured: false });
  render(<FanartSettingsPanel />);
  fireEvent.click(await screen.findByRole("button", { name: "Remove fanart.tv credentials" }));
  await waitFor(() => expect(save).toHaveBeenCalledWith({ mode: "clear" }));
  expect(await screen.findByText("Saved fanart.tv credentials removed.")).toBeInTheDocument();
});
