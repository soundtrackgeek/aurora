import type { ComponentProps } from "react";
import { ContentTransition } from "../../components/ContentTransition";
import { RatingsStudio } from "../../components/ratings/RatingsStudio";
import { TonightsAlbum } from "../../components/ratings/TonightsAlbum";

export type RatingsRouteProps = ComponentProps<typeof RatingsStudio> & {
  tonight: ComponentProps<typeof TonightsAlbum>;
};

export default function RatingsRoute({ tonight, ...studio }: RatingsRouteProps) {
  return <ContentTransition type="collection">
    <TonightsAlbum {...tonight} />
    <RatingsStudio {...studio} />
  </ContentTransition>;
}

export { RatingAlbumInspector } from "../../components/ratings/RatingsStudio";
