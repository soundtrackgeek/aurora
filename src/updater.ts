import { useCallback, useEffect, useRef, useState } from "react";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { isTauriRuntime } from "./library";

const UPDATE_INTERVAL_MS = 60_000;

export type UpdatePhase = "idle" | "available" | "downloading" | "installing" | "error";

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
  const promptedVersionsRef = useRef(new Set<string>());

  const checkForUpdate = useCallback(async () => {
    if (!isTauriRuntime() || import.meta.env.DEV || checkingRef.current) return;
    checkingRef.current = true;

    try {
      const update = await check({ timeout: 15_000 });
      if (!update) return;

      if (updateRef.current && updateRef.current.version !== update.version) {
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
        isPromptOpen: firstPrompt,
      });
    } catch (error) {
      console.warn("Aurora update check failed", error);
    } finally {
      checkingRef.current = false;
    }
  }, []);

  useEffect(() => {
    void checkForUpdate();
    const timer = window.setInterval(() => void checkForUpdate(), UPDATE_INTERVAL_MS);
    return () => {
      window.clearInterval(timer);
      const update = updateRef.current;
      updateRef.current = null;
      if (update) void update.close();
    };
  }, [checkForUpdate]);

  const install = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;

    let downloaded = 0;
    let total: number | null = null;
    setState((current) => ({ ...current, phase: "downloading", progress: 0 }));

    try {
      await update.downloadAndInstall((event: DownloadEvent) => {
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
    } catch (error) {
      setState((current) => ({
        ...current,
        phase: "error",
        progress: null,
        message: error instanceof Error ? error.message : String(error),
        isPromptOpen: true,
      }));
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
