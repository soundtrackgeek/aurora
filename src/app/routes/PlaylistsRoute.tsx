import type { ComponentProps } from "react";
import { PlaylistsPage } from "../../components/playlists/PlaylistsPage";

export type PlaylistsRouteProps = ComponentProps<typeof PlaylistsPage>;

export default function PlaylistsRoute(props: PlaylistsRouteProps) {
  return <PlaylistsPage {...props} />;
}
