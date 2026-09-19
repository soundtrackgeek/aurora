import { ViewTransition, type ReactNode } from "react";
import type { ContentTransitionType } from "../contentTransition";
import "./ContentTransition.css";

/** Scope snapshots to navigation/content updates; ordinary edits remain immediate. */
export function ContentTransition({
  children,
  type,
  enter = false,
  exit = false,
  name,
}: {
  children: ReactNode;
  type?: ContentTransitionType;
  enter?: boolean;
  exit?: boolean;
  name?: string;
}) {
  return (
    <ViewTransition
      name={name}
      share={name ? (type ? { [type]: "aurora-content", default: "none" } : "aurora-content") : undefined}
      default="none"
      exit={exit ? (type ? { [type]: "aurora-content", default: "none" } : "aurora-content") : undefined}
      enter={enter ? (type ? { [type]: "aurora-content", default: "aurora-content" } : "aurora-content") : undefined}
      update={type ? { [type]: "aurora-content", default: "none" } : (enter ? "none" : "aurora-content")}
    >
      {children}
    </ViewTransition>
  );
}
