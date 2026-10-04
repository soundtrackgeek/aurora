import type { ComponentProps } from "react";
import { ListeningHistory } from "../../components/history/ListeningHistory";

export type HistoryRouteProps = ComponentProps<typeof ListeningHistory>;

export default function HistoryRoute(props: HistoryRouteProps) {
  return <ListeningHistory {...props} />;
}
