import type { ComponentProps } from "react";
import { ContentTransition } from "../../components/ContentTransition";
import { RatingsStudio } from "../../components/ratings/RatingsStudio";
import { TonightsAlbum } from "../../components/ratings/TonightsAlbum";
import { HiddenFavorites } from "../../components/HiddenFavorites";

export type RatingsRouteProps = ComponentProps<typeof RatingsStudio> & {
  tonight: ComponentProps<typeof TonightsAlbum>;
  hiddenFavorites: ComponentProps<typeof HiddenFavorites>;
};

export default function RatingsRoute({ tonight, hiddenFavorites, ...studio }: RatingsRouteProps) {
  return <ContentTransition type="collection">
    <TonightsAlbum {...tonight} />
    <HiddenFavorites {...hiddenFavorites} />
    <RatingsStudio {...studio} />
  </ContentTransition>;
}

export { RatingAlbumInspector } from "../../components/ratings/RatingsStudio";
