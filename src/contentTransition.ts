import { addTransitionType, startTransition } from "react";

export type ContentTransitionType = "history-page" | "chart-page" | "album-detail";

/** Keep updates immediate on older WebViews and when motion is unwanted. */
export function transitionContent(update: () => void, type?: ContentTransitionType) {
  if (
    typeof document === "undefined"
    || typeof document.startViewTransition !== "function"
    || window.matchMedia?.("(prefers-reduced-motion: reduce)").matches
  ) {
    update();
    return;
  }
  startTransition(() => {
    if (type) addTransitionType(type);
    update();
  });
}
