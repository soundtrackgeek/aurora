import { ViewTransition, type ReactNode } from "react";
import type { ContentTransitionType } from "../contentTransition";
import "./ContentTransition.css";

/** Animate completed content updates and optional Suspense reveals, never entry or removal of a spinner. */
export function ContentTransition({
  children,
  type,
  enter = false,
}: {
  children: ReactNode;
  type?: ContentTransitionType;
  enter?: boolean;
}) {
  return (
    <ViewTransition
      default="none"
      enter={enter ? (type ? { [type]: "aurora-content", default: "aurora-content" } : "aurora-content") : undefined}
      update={type ? { [type]: "aurora-content", default: "none" } : "aurora-content"}
    >
      {children}
    </ViewTransition>
  );
}
