import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { TonehavnSettingsPanel } from "./TonehavnSettingsPanel";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);
const signedOut = { server: "https://pc.ts.net", signedIn: false, savedSession: false, expiresAt: null, message: "Not signed in." };
beforeEach(() => { mockInvoke.mockReset(); });
afterEach(cleanup);
describe("Tonehavn authentication settings", () => {
  it("clears the password and code after rejected login and allows a fresh attempt", async () => {
    mockInvoke.mockResolvedValueOnce(signedOut).mockRejectedValueOnce("Sign-in rejected");
    render(<TonehavnSettingsPanel />);
    await screen.findByText("Not signed in.");
    fireEvent.change(screen.getByLabelText("Tonehavn username"), { target: { value: "owner" } });
    fireEvent.change(screen.getByLabelText("Tonehavn password"), { target: { value: "fixture-password" } });
    fireEvent.change(screen.getByLabelText("Authenticator code"), { target: { value: "123456" } });
    fireEvent.click(screen.getByRole("button", { name: "Sign in to Tonehavn" }));
    await screen.findByRole("alert");
    expect(screen.getByLabelText("Tonehavn password")).toHaveValue("");
    expect(screen.getByLabelText("Authenticator code")).toHaveValue("");
    expect(mockInvoke).toHaveBeenLastCalledWith("tonehavn_login", { request: { server: signedOut.server, username: "owner", password: "fixture-password", totpCode: "123456" } });
  });
  it("keeps saved-session controls available when the server is offline", async () => {
    mockInvoke.mockResolvedValueOnce({ ...signedOut, savedSession: true, message: "Server unavailable" })
      .mockResolvedValueOnce(signedOut);
    render(<TonehavnSettingsPanel />);
    await screen.findByText("Server unavailable");
    expect(screen.queryByLabelText("Tonehavn password")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Tonehavn server address")).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Sign out of Tonehavn" }));
    await waitFor(() => expect(screen.getByLabelText("Tonehavn password")).toBeInTheDocument());
    expect(mockInvoke).toHaveBeenLastCalledWith("tonehavn_logout", undefined);
  });
});
