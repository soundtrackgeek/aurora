import type { ComponentProps } from "react";
import { ArtistPage } from "../../components/artist/ArtistPage";
import { useAppSlice } from "../useAppStore";

export type ArtistRouteProps = Omit<ComponentProps<typeof ArtistPage>, "catalogRevision">;

export default function ArtistRoute(props: ArtistRouteProps) {
  const catalogRevision = useAppSlice("catalogRevision");
  return <ArtistPage {...props} catalogRevision={catalogRevision} />;
}
