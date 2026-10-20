import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { TonehavnSettingsPanel } from "./TonehavnSettingsPanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);
const signedOut = { server: "https://pc.ts.net", signedIn: false, savedSession: false,
  expiresAt: null, pendingEmailExpiresAt: null, message: "Not signed in." };
const pending = { ...signedOut, pendingEmailExpiresAt: Date.now() + 900_000,
  message: "Check your email for a sign-in link." };
const connected = { ...signedOut, signedIn: true, savedSession: true,
  expiresAt: Date.now() + 315_360_000_000, message: "Connected as a trusted device." };
const link = `${signedOut.server}/auth/email#token=${"a".repeat(64)}`;

beforeEach(() => { mockInvoke.mockReset(); });
afterEach(cleanup);

describe("Tonehavn email-link sign-in", () => {
  it("requests a link using only email and redeems it separately", async () => {
    mockInvoke.mockResolvedValueOnce(signedOut).mockResolvedValueOnce(pending).mockResolvedValueOnce(connected);
    render(<TonehavnSettingsPanel />);
    await screen.findByText("Not signed in.");
    expect(screen.queryByLabelText("Tonehavn password")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Tonehavn username")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Authenticator code")).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Tonehavn email"), { target: { value: "owner@example.com" } });
    fireEvent.submit(screen.getByRole("button", { name: "Send sign-in link" }).closest("form")!);
    await screen.findByLabelText("Sign-in link");
    expect(mockInvoke).toHaveBeenLastCalledWith("tonehavn_request_email_login", {
      request: { server: signedOut.server, email: "owner@example.com" },
    });
    expect(screen.getByLabelText("Tonehavn server address")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Complete sign-in" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Sign-in link"), { target: { value: link } });
    fireEvent.submit(screen.getByRole("button", { name: "Complete sign-in" }).closest("form")!);
    await screen.findByText("Connected as a trusted device.");
    expect(mockInvoke).toHaveBeenLastCalledWith("tonehavn_complete_email_login", { request: { link } });
    expect(screen.queryByLabelText("Sign-in link")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Verify saved session" })).toBeEnabled();
  });

  it("restores a pending request after reopening and can start again with another address", async () => {
    mockInvoke.mockResolvedValueOnce(pending).mockResolvedValueOnce(signedOut);
    render(<TonehavnSettingsPanel />);
    await screen.findByLabelText("Sign-in link");
    expect(screen.queryByLabelText("Tonehavn email")).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Sign-in link"), { target: { value: link } });
    fireEvent.click(screen.getByRole("button", { name: "Start again" }));
    await screen.findByLabelText("Tonehavn email");
    expect(screen.getByLabelText("Tonehavn server address")).toBeEnabled();
    expect(mockInvoke).toHaveBeenLastCalledWith("tonehavn_cancel_email_login", undefined);
  });

  it("clears a rejected link and returns to email entry when the request expired", async () => {
    mockInvoke.mockResolvedValueOnce(pending).mockRejectedValueOnce("This email link expired.")
      .mockResolvedValueOnce(signedOut);
    render(<TonehavnSettingsPanel />);
    await screen.findByLabelText("Sign-in link");
    fireEvent.change(screen.getByLabelText("Sign-in link"), { target: { value: link } });
    fireEvent.click(screen.getByRole("button", { name: "Complete sign-in" }));
    await screen.findByLabelText("Tonehavn email");
    expect(screen.getByRole("alert")).toHaveTextContent("This email link expired.");
    expect(screen.queryByLabelText("Sign-in link")).not.toBeInTheDocument();
    expect(mockInvoke).toHaveBeenLastCalledWith("tonehavn_status");
  });

  it("keeps an unredeemed request after an invalid link or temporary connection failure", async () => {
    mockInvoke.mockResolvedValueOnce(pending).mockRejectedValueOnce("Check the link and retry.")
      .mockResolvedValueOnce(pending);
    render(<TonehavnSettingsPanel />);
    await screen.findByLabelText("Sign-in link");
    fireEvent.change(screen.getByLabelText("Sign-in link"), { target: { value: "invalid link" } });
    fireEvent.click(screen.getByRole("button", { name: "Complete sign-in" }));
    await screen.findByRole("alert");
    await waitFor(() => expect(screen.getByRole("button", { name: "Start again" })).toBeEnabled());
    expect(screen.getByLabelText("Sign-in link")).toHaveValue("");
    expect(screen.getByRole("alert")).toHaveTextContent("Check the link and retry.");
  });

  it("recovers a possibly delivered link when requesting email times out", async () => {
    mockInvoke.mockResolvedValueOnce(signedOut).mockRejectedValueOnce("Could not confirm email delivery.")
      .mockResolvedValueOnce(pending);
    render(<TonehavnSettingsPanel />);
    await screen.findByLabelText("Tonehavn email");
    fireEvent.change(screen.getByLabelText("Tonehavn email"), { target: { value: "owner@example.com" } });
    fireEvent.click(screen.getByRole("button", { name: "Send sign-in link" }));
    await screen.findByLabelText("Sign-in link");
    expect(screen.getByRole("alert")).toHaveTextContent("Could not confirm email delivery.");
  });

  it("keeps saved-session controls available when the server is offline", async () => {
    mockInvoke.mockResolvedValueOnce({ ...signedOut, savedSession: true, message: "Server unavailable" })
      .mockResolvedValueOnce(signedOut);
    render(<TonehavnSettingsPanel />);
    await screen.findByText("Server unavailable");
    expect(screen.queryByLabelText("Tonehavn email")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Tonehavn server address")).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Sign out of Tonehavn" }));
    await screen.findByLabelText("Tonehavn email");
    expect(mockInvoke).toHaveBeenLastCalledWith("tonehavn_logout", undefined);
  });

  it("returns to email sign-in when a saved session is revoked", async () => {
    mockInvoke.mockResolvedValueOnce(connected).mockResolvedValueOnce({ ...signedOut, message: "Session revoked. Sign in again." });
    render(<TonehavnSettingsPanel />);
    await screen.findByRole("button", { name: "Verify saved session" });
    fireEvent.click(screen.getByRole("button", { name: "Verify saved session" }));
    await screen.findByLabelText("Tonehavn email");
    expect(screen.getByRole("status")).toHaveTextContent("Session revoked.");
  });
});
