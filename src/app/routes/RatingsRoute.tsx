import type { ComponentProps } from "react";
import { ContentTransition } from "../../components/ContentTransition";
import { RatingsStudio } from "../../components/ratings/RatingsStudio";
import { TonightsAlbum } from "../../components/ratings/TonightsAlbum";
import { HiddenFavorites } from "../../components/HiddenFavorites";
import { RatingMissions } from "../../components/RatingMissions";

export type RatingsRouteProps = ComponentProps<typeof RatingsStudio> & {
  tonight: ComponentProps<typeof TonightsAlbum>;
  hiddenFavorites: ComponentProps<typeof HiddenFavorites>;
  ratingMissions: ComponentProps<typeof RatingMissions>;
};

export default function RatingsRoute({ tonight, hiddenFavorites, ratingMissions, ...studio }: RatingsRouteProps) {
  return <ContentTransition type="collection">
    <TonightsAlbum {...tonight} />
    <HiddenFavorites {...hiddenFavorites} />
    <RatingMissions {...ratingMissions} />
    <RatingsStudio {...studio} />
  </ContentTransition>;
}

export { RatingAlbumInspector } from "../../components/ratings/RatingsStudio";
