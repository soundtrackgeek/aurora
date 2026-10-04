import type { ComponentProps } from "react";
import { ContentTransition } from "../../components/ContentTransition";
import { DeepExplorer } from "../../components/explorer/DeepExplorer";
import { useAppSlice } from "../useAppStore";

export type LibraryRouteProps = Omit<ComponentProps<typeof DeepExplorer>, "selectedTrackId">;

export default function LibraryRoute(props: LibraryRouteProps) {
  const selectedTrack = useAppSlice("selectedTrack");
  return <ContentTransition type="artist-detail"><DeepExplorer {...props} selectedTrackId={selectedTrack?.id ?? null} /></ContentTransition>;
}
