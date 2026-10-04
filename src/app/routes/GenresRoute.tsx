import type { ComponentProps } from "react";
import { ContentTransition } from "../../components/ContentTransition";
import { GenreAtlas } from "../../components/genres/GenreAtlas";

export type GenresRouteProps = ComponentProps<typeof GenreAtlas>;

export default function GenresRoute(props: GenresRouteProps) {
  return <ContentTransition type="collection"><GenreAtlas {...props} /></ContentTransition>;
}
