import { ViewTransition, type ReactNode } from "react";
import type { ContentTransitionType } from "../contentTransition";
import "./ContentTransition.css";

/** Only animate completed content updates, never entry or removal of a spinner. */
export function ContentTransition({ children, type }: { children: ReactNode; type?: ContentTransitionType }) {
  return <ViewTransition default="none" update={type ? { [type]: "aurora-content", default: "none" } : "aurora-content"}>{children}</ViewTransition>;
}
