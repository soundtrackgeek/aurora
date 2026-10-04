import type { ComponentProps } from "react";
import { ContentTransition } from "../../components/ContentTransition";
import { YearsExplorer } from "../../components/library/YearsExplorer";

export type YearsRouteProps = ComponentProps<typeof YearsExplorer>;

export default function YearsRoute(props: YearsRouteProps) {
  return <ContentTransition type="collection"><YearsExplorer {...props} /></ContentTransition>;
}

export { YearAlbumInspector } from "../../components/library/YearsExplorer";
