import type { ComponentProps } from "react";
import { Observatory } from "../../components/curation/Observatory";

export type ObservatoryRouteProps = ComponentProps<typeof Observatory>;

export default function ObservatoryRoute(props: ObservatoryRouteProps) {
  return <Observatory {...props} />;
}
