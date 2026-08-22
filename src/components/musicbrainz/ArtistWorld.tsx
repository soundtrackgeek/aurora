import {
  AlertTriangle,
  BadgeCheck,
  Database,
  Disc3,
  LibraryBig,
  LoaderCircle,
  Orbit,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import { useMemo } from "react";
import { formatCount, type ArtistDetail } from "../../library";
import type { ArtistIntelligence, MusicBrainzMatchState } from "../../musicbrainz";
import "./ArtistWorld.css";

export type ArtistWorldState = "loading" | "ready" | "error";

export interface ArtistWorldProps {
  artistName: string;
  catalogDetail: ArtistDetail | null;
  intelligence: ArtistIntelligence | null;
  state: ArtistWorldState;
  errorMessage?: string | null;
  onRetry: () => void;
  onExploreLibrary: () => void;
}

const matchCopy: Record<MusicBrainzMatchState, { label: string; detail: string; tone: string }> = {
  verified: {
    label: "Curated identity",
    detail: "A verified local overlay link identifies this artist.",
    tone: "verified",
  },
  unconfirmed: {
    label: "Unconfirmed identity",
    detail: "The catalog or broad cache has an exact-name candidate. It is useful, but not manually verified.",
    tone: "discovered",
  },
  conflict: {
    label: "Identity conflict",
    detail: "Local identity sources disagree. Aurora will not guess; a verified curated link is shown only when one exists.",
    tone: "warning",
  },
  unmatched: {
    label: "Not connected",
    detail: "No exact or curated local MusicBrainz identity was found.",
    tone: "quiet",
  },
  ignored: {
    label: "Intentionally ignored",
    detail: "The curated overlay marks this artist as ignored, so Aurora will not guess.",
    tone: "quiet",
  },
};

function releaseKind(primary: string | null, secondary: readonly string[]): string {
  return [...(primary ? [primary] : []), ...secondary].join(" · ") || "Release group";
}

function provenanceLabel(provenance: ArtistIntelligence["releases"][number]["provenance"]): string {
  if (provenance === "curatedOverlay") return "Curated overlay";
  if (provenance === "catalogImport") return "Catalog mirror";
  return "Broad cache";
}

function identityProvenanceLabel(provenance: NonNullable<ArtistIntelligence["identity"]>["provenance"]): string {
  if (provenance === "curatedOverlay") return "Curated overlay";
  if (provenance === "catalogOverlay") return "Catalog overlay mirror";
  if (provenance === "catalogImport") return "Catalog import";
  return "Exact broad-cache name";
}

function activeSpan(intelligence: ArtistIntelligence): string {
  const profile = intelligence.profile;
  if (!profile?.lifeBeginDate) return "Unknown";
  if (profile.lifeEndDate) return `${profile.lifeBeginDate}–${profile.lifeEndDate}`;
  return profile.lifeEnded ? `${profile.lifeBeginDate}–ended` : `${profile.lifeBeginDate}–present`;
}

export function ArtistWorld({
  artistName,
  catalogDetail,
  intelligence,
  state,
  errorMessage,
  onRetry,
  onExploreLibrary,
}: ArtistWorldProps) {
  const match = intelligence ? matchCopy[intelligence.matchState] : null;
  const constellation = useMemo(() => intelligence?.releases.slice(0, 8) ?? [], [intelligence]);

  return (
    <div className="artist-world">
      <header className="artist-world__hero">
        <div className="artist-world__orbit" aria-hidden="true">
          <span className="artist-world__sun" />
          {constellation.map((release, index) => (
            <span className={`artist-world__planet artist-world__planet--${index + 1}`} key={release.mbid} />
          ))}
        </div>
        <p className="eyebrow"><Sparkles aria-hidden="true" /> Constellations</p>
        <h2>{artistName}</h2>
        <p>Local catalog gravity with optional MusicBrainz context.</p>
      </header>

      <button type="button" className="artist-world__library" onClick={onExploreLibrary}>
        <LibraryBig aria-hidden="true" /> Explore this artist in Aurora
      </button>

      {catalogDetail ? (
        <dl className="artist-world__stats" aria-label="Local catalog summary">
          <div><dt>Tracks</dt><dd>{formatCount(catalogDetail.artist.trackCount)}</dd></div>
          <div><dt>Albums</dt><dd>{formatCount(catalogDetail.artist.albumCount)}</dd></div>
          <div><dt>Loaded</dt><dd>{formatCount(catalogDetail.albums.length)}</dd></div>
        </dl>
      ) : null}

      {state === "ready" && errorMessage ? (
        <div className="artist-world__degraded" role="status">
          <AlertTriangle aria-hidden="true" /><span><strong>Partial local context</strong>{errorMessage}</span>
        </div>
      ) : null}

      {state === "loading" ? (
        <div className="artist-world__feedback" role="status">
          <LoaderCircle className="is-spinning" aria-hidden="true" />
          <span><strong>Tracing local connections…</strong>No online request is being made.</span>
        </div>
      ) : state === "error" ? (
        <div className="artist-world__feedback is-error" role="alert">
          <AlertTriangle aria-hidden="true" />
          <span><strong>Context could not be opened</strong>{errorMessage ?? "The catalog remains available."}</span>
          <button type="button" onClick={onRetry}><RefreshCw aria-hidden="true" /> Retry</button>
        </div>
      ) : intelligence && match ? (
        <>
          <section className={`artist-world__identity is-${match.tone}`} aria-label="MusicBrainz identity status">
            {intelligence.matchState === "verified" ? <BadgeCheck aria-hidden="true" /> : intelligence.matchState === "unmatched" || intelligence.matchState === "ignored" ? <Orbit aria-hidden="true" /> : <AlertTriangle aria-hidden="true" />}
            <div>
              <strong>{match.label}</strong>
              <p>{match.detail}</p>
              {intelligence.identity ? <code title={intelligence.identity.mbid}>{intelligence.identity.mbid}</code> : null}
              {intelligence.identity ? <small className="artist-world__provenance">{identityProvenanceLabel(intelligence.identity.provenance)} · {intelligence.identity.matchMethod}</small> : null}
              {intelligence.identity && intelligence.identity.cacheNameCount && intelligence.identity.cacheNameCount > 1 ? <p className="artist-world__ambiguity">This cache MBID is shared by {formatCount(intelligence.identity.cacheNameCount)} artist names.</p> : null}
            </div>
          </section>

          {intelligence.profile ? (
            <dl className="artist-world__profile" aria-label="MusicBrainz artist profile">
              <div><dt>Type</dt><dd>{intelligence.profile.artistType ?? "Unknown"}</dd></div>
              <div><dt>Active</dt><dd>{activeSpan(intelligence)}</dd></div>
              <div><dt>Country</dt><dd>{intelligence.profile.countryName ?? "Unknown"}</dd></div>
              <div><dt>Area</dt><dd>{intelligence.profile.areaName ?? "Unknown"}</dd></div>
              <div><dt>Begun in</dt><dd>{intelligence.profile.beginAreaName ?? "Unknown"}</dd></div>
              <div><dt>Ended in</dt><dd>{intelligence.profile.endAreaName ?? "Unknown"}</dd></div>
            </dl>
          ) : null}

          <section className="artist-world__sources" aria-labelledby="artist-world-sources">
            <h3 id="artist-world-sources">Local sources</h3>
            {intelligence.sources.map((source) => (
              <div key={source.id}>
                <Database aria-hidden="true" />
                <span><strong>{source.label}</strong><small>{source.detail}</small></span>
                <i className={`source-light is-${source.status}`} aria-label={source.status} />
              </div>
            ))}
          </section>

          <section className="artist-world__releases" aria-labelledby="artist-world-releases">
            <div className="artist-world__section-heading">
              <div><p className="eyebrow">Release groups</p><h3 id="artist-world-releases">Connected worlds</h3></div>
              <span>{formatCount(intelligence.releases.length)}{intelligence.releasesTruncated ? "+" : ""}</span>
            </div>
            {intelligence.releases.length ? (
              <ol>
                {intelligence.releases.map((release) => (
                  <li key={release.mbid}>
                    <span className="release-world" aria-hidden="true"><Disc3 /></span>
                    <div>
                      <strong>{release.title}</strong>
                      <small>{release.year ?? "Year unknown"} · {releaseKind(release.primaryType, release.secondaryTypes)}</small>
                      <small>{provenanceLabel(release.provenance)}{release.status ? ` · ${release.status}` : ""}</small>
                    </div>
                    {release.decision ? <em>{release.decision}</em> : null}
                  </li>
                ))}
              </ol>
            ) : (
              <p className="artist-world__empty">No release groups are stored for this local identity.</p>
            )}
            {intelligence.releasesTruncated ? <p className="artist-world__bounded">Showing the newest 100 release groups.</p> : null}
          </section>
        </>
      ) : null}
    </div>
  );
}
