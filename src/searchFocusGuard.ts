type FocusRestoreScheduler = (restore: () => void) => void;

export function isWindowsTauriWebView(): boolean {
  return typeof window !== "undefined"
    && "__TAURI_INTERNALS__" in window
    && /Windows/u.test(window.navigator.userAgent);
}

export function preparePopulatedInputForFocus(
  input: HTMLInputElement,
  enabled = isWindowsTauriWebView(),
  schedule: FocusRestoreScheduler = (restore) => window.requestAnimationFrame(restore),
): boolean {
  if (!enabled || document.activeElement === input || input.value === "") return false;

  const value = input.value;
  input.value = "";
  schedule(() => {
    if (!input.isConnected || input.value !== "") return;
    input.value = value;
    if (document.activeElement === input) input.setSelectionRange(value.length, value.length);
  });
  return true;
}
