import {
  AlertTriangle,
  Ban,
  BadgeCheck,
  Check,
  Database,
  Disc3,
  LibraryBig,
  Link2,
  LoaderCircle,
  Orbit,
  RefreshCw,
  Sparkles,
  X,
} from "lucide-react";
import { useMemo, useState } from "react";
import { formatCount, type ArtistDetail } from "../../library";
import type {
  ArtistDecisionRequest,
  ArtistIntelligence,
  MusicBrainzMatchState,
  ReleaseDecisionRequest,
} from "../../musicbrainz";
import "./ArtistWorld.css";

export type ArtistWorldState = "loading" | "ready" | "error";

export interface ArtistWorldProps {
  artistName: string;
  catalogDetail: ArtistDetail | null;
  intelligence: ArtistIntelligence | null;
  state: ArtistWorldState;
  errorMessage?: string | null;
  curationError?: string | null;
  actionBusy?: string | null;
  onRetry: () => void;
  onExploreLibrary: () => void;
  onArtistDecision: (request: ArtistDecisionRequest) => void;
  onReleaseDecision: (request: ReleaseDecisionRequest) => void;
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
  if (provenance === "auroraState") return "Aurora decision";
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
  curationError,
  actionBusy = null,
  onRetry,
  onExploreLibrary,
  onArtistDecision,
  onReleaseDecision,
}: ArtistWorldProps) {
  const match = intelligence ? matchCopy[intelligence.matchState] : null;
  const constellation = useMemo(() => intelligence?.releases.slice(0, 8) ?? [], [intelligence]);
  const [selectedCandidate, setSelectedCandidate] = useState<string>(intelligence?.identity?.mbid ?? intelligence?.candidates[0]?.mbid ?? "");
  const [activeRelease, setActiveRelease] = useState<string | null>(null);
  const [albumChoices, setAlbumChoices] = useState<Record<string, string>>({});
  const effectiveCandidate = selectedCandidate || intelligence?.identity?.mbid || intelligence?.candidates[0]?.mbid || "";

  const authoritativeIdentity = intelligence?.identity && ["auroraState", "curatedOverlay", "catalogOverlay"].includes(intelligence.identity.provenance)
    ? intelligence.identity
    : null;

  function releaseAlbumOptions(releaseTitle: string) {
    const normalized = releaseTitle.trim().toLocaleLowerCase();
    return [...(catalogDetail?.albums ?? [])].sort((left, right) => {
      const leftExact = left.title.trim().toLocaleLowerCase() === normalized;
      const rightExact = right.title.trim().toLocaleLowerCase() === normalized;
      return Number(rightExact) - Number(leftExact) || (right.releaseYear ?? 9999) - (left.releaseYear ?? 9999) || left.title.localeCompare(right.title);
    });
  }

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
              {intelligence.hasExternalConflict && intelligence.decision ? <p className="artist-world__ambiguity">Aurora keeps your decision, while an imported source still disagrees.</p> : null}
            </div>
          </section>

          <section className="artist-world__curation" aria-labelledby="artist-curation-title">
            <div className="artist-world__section-heading">
              <div><p className="eyebrow">Your authority</p><h3 id="artist-curation-title">Identity decision</h3></div>
              {intelligence.decision ? <span>Saved locally</span> : null}
            </div>
            {curationError ? <p className="artist-world__curation-error" role="alert">{curationError}</p> : null}
            {intelligence.decision ? (
              <div className="artist-world__saved-decision">
                <BadgeCheck aria-hidden="true" />
                <span><strong>{intelligence.decision.decision === "confirmed" ? "Aurora confirmed" : "Aurora ignored"}</strong><small>This override survives restart in aurora-state.sqlite3.</small></span>
                <button
                  type="button"
                  disabled={actionBusy !== null}
                  onClick={() => onArtistDecision({ action: "clear", artist: artistName })}
                >{actionBusy === "artist" ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <X aria-hidden="true" />} Clear</button>
              </div>
            ) : (
              <>
                {intelligence.candidates.length ? (
                  <fieldset className="artist-world__candidates">
                    <legend>Local candidates</legend>
                    {intelligence.candidates.map((candidate, index) => (
                      <label key={`${candidate.provenance}:${candidate.mbid}:${index}`}>
                        <input
                          type="radio"
                          name={`artist-candidate-${intelligence.artistKey}`}
                          value={candidate.mbid}
                          checked={effectiveCandidate === candidate.mbid}
                          onChange={() => setSelectedCandidate(candidate.mbid)}
                        />
                        <span><strong>{candidate.canonicalName}</strong><code>{candidate.mbid}</code><small>{identityProvenanceLabel(candidate.provenance)} · {candidate.matchMethod}{candidate.verifiedSource ? " · verified source" : ""}</small></span>
                      </label>
                    ))}
                  </fieldset>
                ) : <p className="artist-world__empty">No local candidate is available to confirm. You can still ignore this artist.</p>}
                <div className="artist-world__curation-actions">
                  <button
                    type="button"
                    className="is-primary"
                    disabled={!effectiveCandidate || actionBusy !== null}
                    onClick={() => onArtistDecision({ action: "confirm", artist: artistName, mbid: effectiveCandidate })}
                  >{actionBusy === "artist" ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Check aria-hidden="true" />} Confirm candidate</button>
                  <button
                    type="button"
                    disabled={actionBusy !== null}
                    onClick={() => onArtistDecision({ action: "ignore", artist: artistName })}
                  ><Ban aria-hidden="true" /> Ignore artist</button>
                </div>
              </>
            )}
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
                    <button
                      type="button"
                      className="release-curate"
                      disabled={!authoritativeIdentity || actionBusy !== null}
                      onClick={() => setActiveRelease((current) => current === release.mbid ? null : release.mbid)}
                    >{release.decision ? release.decision : "Curate"}</button>
                    {activeRelease === release.mbid && authoritativeIdentity ? (
                      <div className="release-editor">
                        <label>
                          Local album
                          <select
                            value={albumChoices[release.mbid] ?? release.localAlbumId ?? ""}
                            onChange={(event) => setAlbumChoices((current) => ({ ...current, [release.mbid]: event.target.value }))}
                          >
                            <option value="">Choose an album…</option>
                            {releaseAlbumOptions(release.title).map((album) => (
                              <option value={album.id} key={album.id}>{album.title} · {album.releaseYear ?? "year unknown"}</option>
                            ))}
                          </select>
                        </label>
                        {(() => {
                          const albumId = albumChoices[release.mbid] ?? release.localAlbumId ?? "";
                          return albumId && !albumId.startsWith("mb:") ? <small className="release-editor__portability">This catalog ID may be machine-specific.</small> : null;
                        })()}
                        <div>
                          <button
                            type="button"
                            className="is-primary"
                            disabled={!(albumChoices[release.mbid] ?? release.localAlbumId) || actionBusy !== null}
                            onClick={() => onReleaseDecision({
                              action: "link",
                              artist: artistName,
                              artistMbid: authoritativeIdentity.mbid,
                              releaseMbid: release.mbid,
                              localAlbumId: albumChoices[release.mbid] ?? release.localAlbumId ?? "",
                            })}
                          ><Link2 aria-hidden="true" /> Link</button>
                          <button
                            type="button"
                            disabled={actionBusy !== null}
                            onClick={() => onReleaseDecision({ action: "notInScope", artist: artistName, artistMbid: authoritativeIdentity.mbid, releaseMbid: release.mbid })}
                          ><Ban aria-hidden="true" /> Not in scope</button>
                          {release.decision ? (
                            <button
                              type="button"
                              disabled={actionBusy !== null}
                              onClick={() => onReleaseDecision({ action: "clear", artist: artistName, artistMbid: authoritativeIdentity.mbid, releaseMbid: release.mbid })}
                            ><X aria-hidden="true" /> Clear</button>
                          ) : null}
                        </div>
                        {release.decisionProvenance ? <small>Decision from {release.decisionProvenance === "auroraState" ? "Aurora state" : "curated overlay"}.</small> : null}
                      </div>
                    ) : null}
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
