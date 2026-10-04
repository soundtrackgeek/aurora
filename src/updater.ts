import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { saveWindowState, StateFlags } from "@tauri-apps/plugin-window-state";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { isTauriRuntime } from "./library";

const UPDATE_INTERVAL_MS = 6 * 60 * 60_000;
const UPDATE_FOCUS_INTERVAL_MS = 60 * 60_000;
const UPDATE_RETRY_BASE_MS = 5 * 60_000;

export type UpdatePhase = "idle" | "checking" | "upToDate" | "available" | "downloading" | "installing" | "error";

export interface UpdateState {
  phase: UpdatePhase;
  version: string | null;
  progress: number | null;
  message: string | null;
  isPromptOpen: boolean;
}

const initialState: UpdateState = {
  phase: "idle",
  version: null,
  progress: null,
  message: null,
  isPromptOpen: false,
};

export function useAuroraUpdater() {
  const [state, setState] = useState(initialState);
  const updateRef = useRef<Update | null>(null);
  const checkingRef = useRef(false);
  const installingRef = useRef(false);
  const promptedVersionsRef = useRef(new Set<string>());
  const lastAttemptRef = useRef<number | null>(null);
  const nextAttemptRef = useRef(0);
  const failuresRef = useRef(0);
  const scheduleRef = useRef<() => void>(() => undefined);

  const checkForUpdate = useCallback(async (manual = false) => {
    if (!isTauriRuntime() || import.meta.env.DEV || checkingRef.current || installingRef.current) return;
    checkingRef.current = true;
    lastAttemptRef.current = Date.now();
    if (manual) setState({ ...initialState, phase: "checking", isPromptOpen: true });

    try {
      const update = await check({ timeout: 15_000 });
      failuresRef.current = 0;
      nextAttemptRef.current = Date.now() + UPDATE_INTERVAL_MS;
      if (!update) {
        if (updateRef.current) await updateRef.current.close();
        updateRef.current = null;
        setState({ ...initialState, phase: "upToDate", message: "You’re running the latest Aurora version.", isPromptOpen: manual });
        return;
      }

      if (updateRef.current) {
        await updateRef.current.close();
      }
      updateRef.current = update;
      const firstPrompt = !promptedVersionsRef.current.has(update.version);
      promptedVersionsRef.current.add(update.version);
      setState({
        phase: "available",
        version: update.version,
        progress: null,
        message: update.body ?? null,
        isPromptOpen: manual || firstPrompt,
      });
    } catch (error) {
      failuresRef.current += 1;
      nextAttemptRef.current = Date.now() + Math.min(
        UPDATE_INTERVAL_MS, UPDATE_RETRY_BASE_MS * 2 ** Math.min(failuresRef.current - 1, 10),
      );
      console.warn("Aurora update check failed", error);
      if (manual) setState({ ...initialState, phase: "error", message: error instanceof Error ? error.message : String(error), isPromptOpen: true });
    } finally {
      checkingRef.current = false;
      scheduleRef.current();
    }
  }, []);

  useEffect(() => {
    if (!isTauriRuntime() || import.meta.env.DEV) return;
    let timer: number | undefined;
    const schedule = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => void checkForUpdate(), Math.max(0, nextAttemptRef.current - Date.now()));
    };
    scheduleRef.current = schedule;
    const onFocus = () => {
      if (lastAttemptRef.current === null
        || (Date.now() - lastAttemptRef.current >= UPDATE_FOCUS_INTERVAL_MS
          && (failuresRef.current === 0 || Date.now() >= nextAttemptRef.current))) {
        void checkForUpdate();
      }
    };
    void checkForUpdate();
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("focus", onFocus);
      scheduleRef.current = () => undefined;
      const update = updateRef.current;
      updateRef.current = null;
      if (update) void update.close();
    };
  }, [checkForUpdate]);

  const install = useCallback(async () => {
    const update = updateRef.current;
    if (!update || installingRef.current || checkingRef.current) return;
    installingRef.current = true;

    let playbackPrepared = false;
    let downloaded = 0;
    let total: number | null = null;
    setState((current) => ({ ...current, phase: "downloading", progress: 0 }));

    try {
      await update.download((event: DownloadEvent) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? null;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          const progress = total ? Math.min(100, Math.round((downloaded / total) * 100)) : null;
          setState((current) => ({ ...current, phase: "downloading", progress }));
        } else if (event.event === "Finished") {
          setState((current) => ({ ...current, phase: "installing", progress: 100 }));
        }
      });
      await saveWindowState(StateFlags.SIZE | StateFlags.POSITION | StateFlags.MAXIMIZED);
      await invoke("prepare_playback_shutdown");
      playbackPrepared = true;
      await update.install();
      await invoke("restart_after_update");
    } catch (error) {
      if (playbackPrepared) {
        await invoke("cancel_playback_shutdown").catch((cancelError) => {
          console.warn("Aurora could not release its playback exit checkpoint", cancelError);
        });
      }
      setState((current) => ({
        ...current,
        phase: "error",
        progress: null,
        message: error instanceof Error ? error.message : String(error),
        isPromptOpen: true,
      }));
    } finally {
      installingRef.current = false;
    }
  }, []);

  const dismiss = useCallback(() => {
    setState((current) => ({ ...current, isPromptOpen: false }));
  }, []);

  const showPrompt = useCallback(() => {
    setState((current) => ({ ...current, isPromptOpen: true }));
  }, []);

  return { state, install, dismiss, showPrompt, checkForUpdate };
}
