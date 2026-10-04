import { lazy, Suspense, type ComponentProps } from "react";
import type { ChartInspector as ChartInspectorComponent } from "../../components/charts/ChartStudio";
import type { YearAlbumInspector as YearAlbumInspectorComponent } from "../../components/library/YearsExplorer";
import type { ArtistWorld } from "../../components/musicbrainz/ArtistWorld";
import type { PublisherAlbumInspector as PublisherAlbumInspectorComponent } from "../../components/publishers/PublisherSignalTimeline";
import type { RatingAlbumInspector as RatingAlbumInspectorComponent } from "../../components/ratings/RatingsStudio";
import type { ArtistRouteProps } from "./ArtistRoute";
import type { ChartsRouteProps } from "./ChartsRoute";
import type { GenresRouteProps } from "./GenresRoute";
import type { HistoryRouteProps } from "./HistoryRoute";
import type { LibraryRouteProps } from "./LibraryRoute";
import type { ObservatoryRouteProps } from "./ObservatoryRoute";
import type { PlaylistsRouteProps } from "./PlaylistsRoute";
import type { PublishersRouteProps } from "./PublishersRoute";
import type { RatingsRouteProps } from "./RatingsRoute";
import type { YearsRouteProps } from "./YearsRoute";

const LazyChartsRoute = lazy(() => import("./ChartsRoute"));
const LazyArtistRoute = lazy(() => import("./ArtistRoute"));
const LazyGenresRoute = lazy(() => import("./GenresRoute"));
const LazyHistoryRoute = lazy(() => import("./HistoryRoute"));
const LazyLibraryRoute = lazy(() => import("./LibraryRoute"));
const LazyObservatoryRoute = lazy(() => import("./ObservatoryRoute"));
const LazyPlaylistsRoute = lazy(() => import("./PlaylistsRoute"));
const LazyPublishersRoute = lazy(() => import("./PublishersRoute"));
const LazyRatingsRoute = lazy(() => import("./RatingsRoute"));
const LazyYearsRoute = lazy(() => import("./YearsRoute"));
const LazyArtistInspector = lazy(() => import("./ArtistInspector"));
const LazyChartInspector = lazy(async () => ({ default: (await import("./ChartsRoute")).ChartInspector }));
const LazyPublisherAlbumInspector = lazy(async () => ({ default: (await import("./PublishersRoute")).PublisherAlbumInspector }));
const LazyRatingAlbumInspector = lazy(async () => ({ default: (await import("./RatingsRoute")).RatingAlbumInspector }));
const LazyYearAlbumInspector = lazy(async () => ({ default: (await import("./YearsRoute")).YearAlbumInspector }));

function RouteLoading({ destination }: { destination: string; }) {
  return <section className="inbox-load" role="status" data-route-loading>Opening {destination}…</section>;
}

// Keep these boundaries inside RememberedPage. Its first-visit mounting and
// frozen hidden props continue to control both loading and retained page state.
export function ArtistRoute(props: ArtistRouteProps) {
  return <Suspense fallback={<RouteLoading destination="artist" />}><LazyArtistRoute {...props} /></Suspense>;
}

export function ChartsRoute(props: ChartsRouteProps) {
  return <Suspense fallback={<RouteLoading destination="Charts" />}><LazyChartsRoute {...props} /></Suspense>;
}

export function GenresRoute(props: GenresRouteProps) {
  return <Suspense fallback={<RouteLoading destination="Genres" />}><LazyGenresRoute {...props} /></Suspense>;
}

export function HistoryRoute(props: HistoryRouteProps) {
  return <Suspense fallback={<RouteLoading destination="History" />}><LazyHistoryRoute {...props} /></Suspense>;
}

export function LibraryRoute(props: LibraryRouteProps) {
  return <Suspense fallback={<RouteLoading destination="Library" />}><LazyLibraryRoute {...props} /></Suspense>;
}

export function ObservatoryRoute(props: ObservatoryRouteProps) {
  return <Suspense fallback={<RouteLoading destination="Observatory" />}><LazyObservatoryRoute {...props} /></Suspense>;
}

export function PlaylistsRoute(props: PlaylistsRouteProps) {
  return <Suspense fallback={<RouteLoading destination="Playlists" />}><LazyPlaylistsRoute {...props} /></Suspense>;
}

export function PublishersRoute(props: PublishersRouteProps) {
  return <Suspense fallback={<RouteLoading destination="Publishers" />}><LazyPublishersRoute {...props} /></Suspense>;
}

export function RatingsRoute(props: RatingsRouteProps) {
  return <Suspense fallback={<RouteLoading destination="Ratings" />}><LazyRatingsRoute {...props} /></Suspense>;
}

export function YearsRoute(props: YearsRouteProps) {
  return <Suspense fallback={<RouteLoading destination="Years" />}><LazyYearsRoute {...props} /></Suspense>;
}

export function ArtistInspector(props: ComponentProps<typeof ArtistWorld>) {
  return <Suspense fallback={<RouteLoading destination="artist details" />}><LazyArtistInspector {...props} /></Suspense>;
}

export function ChartInspector(props: ComponentProps<typeof ChartInspectorComponent>) {
  return <Suspense fallback={<RouteLoading destination="chart details" />}><LazyChartInspector {...props} /></Suspense>;
}

export function PublisherAlbumInspector(props: ComponentProps<typeof PublisherAlbumInspectorComponent>) {
  return <Suspense fallback={<RouteLoading destination="album details" />}><LazyPublisherAlbumInspector {...props} /></Suspense>;
}

export function RatingAlbumInspector(props: ComponentProps<typeof RatingAlbumInspectorComponent>) {
  return <Suspense fallback={<RouteLoading destination="album details" />}><LazyRatingAlbumInspector {...props} /></Suspense>;
}

type InspectorAlbum = ComponentProps<typeof YearAlbumInspectorComponent>["album"];
type YearAlbumInspectorProps<T extends InspectorAlbum> = Omit<ComponentProps<typeof YearAlbumInspectorComponent>, "album" | "onPlay"> & {
  album: T;
  onPlay: (album: T) => void;
};

export function YearAlbumInspector<T extends InspectorAlbum>({ album, onPlay, ...props }: YearAlbumInspectorProps<T>) {
  return <Suspense fallback={<RouteLoading destination="album details" />}>
    <LazyYearAlbumInspector {...props} album={album} onPlay={() => onPlay(album)} />
  </Suspense>;
}
