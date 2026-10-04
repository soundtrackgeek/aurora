import type { ComponentProps } from "react";
import { ChartStudio } from "../../components/charts/ChartStudio";
import { useAppSlice } from "../useAppStore";

export type ChartsRouteProps = Omit<ComponentProps<typeof ChartStudio>, "catalogRevision">;

export default function ChartsRoute(props: ChartsRouteProps) {
  const catalogRevision = useAppSlice("catalogRevision");
  return <ChartStudio {...props} catalogRevision={catalogRevision} />;
}

export { ChartInspector } from "../../components/charts/ChartStudio";
