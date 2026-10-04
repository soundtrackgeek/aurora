import type { ComponentProps } from "react";
import { ContentTransition } from "../../components/ContentTransition";
import { PublisherSignalTimeline } from "../../components/publishers/PublisherSignalTimeline";

export type PublishersRouteProps = ComponentProps<typeof PublisherSignalTimeline>;

export default function PublishersRoute(props: PublishersRouteProps) {
  return <ContentTransition type="collection"><PublisherSignalTimeline {...props} /></ContentTransition>;
}

export { PublisherAlbumInspector } from "../../components/publishers/PublisherSignalTimeline";
