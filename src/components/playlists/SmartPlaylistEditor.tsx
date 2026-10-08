import { useState } from "react";
import { emptySharedRequest } from "../../sharedPlaylistRules";
import { saveSmartPlaylist, type SharedPlaylistRequest, type SmartPlaylistSettings } from "../../playlists";

const textRules = [["albumArtist","Album artist"],["displayArtist","Track artist"],["albumTitle","Album title"],["trackTitle","Song title"],["publisher","Publisher"]] as const;

type Props = { id?: number; revision?:string; name?: string; request?: SharedPlaylistRequest; settings?: SmartPlaylistSettings; onSaved: (id: number) => void; onCancel: () => void; warning?: string; onBusyChange?: (busy: boolean) => void };
export function SmartPlaylistEditor({ id, revision, name: initialName = "", request: initialRequest, settings, onSaved, onCancel, warning, onBusyChange }: Props) {
  const [editRevision] = useState(revision);
  const [name, setName] = useState(initialName);
  const [request, setRequest] = useState(initialRequest ?? emptySharedRequest());
  const [limit, setLimit] = useState(settings?.trackLimit ?? 1000);
  const [policy, setPolicy] = useState<SmartPlaylistSettings["refreshPolicy"]>(settings?.refreshPolicy ?? "library");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const update = (key: string, value: unknown) => setRequest(current => ({ ...current, filters: { ...current.filters, [key]: value } }));
  const genres = (request.filters.genres as string[] | undefined) ?? [];
  const ratingKey = request.view === "albums" ? "albumRatingMin" : "trackRatingMin";
  const maximumRatingKey = request.view === "albums" ? "albumRatingMax" : "trackRatingMax";
  const ratingScale = request.view === "albums" ? 20 : 1;
  return <form className="playlist-rule-editor" aria-label="Smart playlist rules" onSubmit={event => {
    event.preventDefault(); if (busy) return; setBusy(true); onBusyChange?.(true); setError(null);
    void saveSmartPlaylist({ id: id ?? null, expectedUpdatedAt:editRevision, name, request, settings: { trackLimit: limit, refreshPolicy: policy } }).then(saved => onSaved(saved.id)).catch((reason: unknown) => setError(String(reason))).finally(() => { setBusy(false); onBusyChange?.(false); });
  }}>
    <h2>{id ? "Edit Smart playlist" : "Create Smart playlist"}</h2>
    <p>Reusable Music Library filters. Other saved filters are retained when editing.</p>
    {warning && <p role="status">{warning} The fields below start a new shared recipe.</p>}
    <fieldset disabled={busy}>
      <label>Name<input required maxLength={120} value={name} onChange={e => setName(e.target.value)} /></label>
      <label>Match<select value={request.view} onChange={e=>setRequest({...request,view:e.target.value as "tracks"|"albums"})}><option value="tracks">Songs</option><option value="albums">Albums — all their songs</option></select></label>
      <label>Match text<input maxLength={256} value={request.searchText} onChange={e => setRequest({ ...request, searchText: e.target.value })} /></label>
      {textRules.map(([key,label])=>{ const rule=(request.filters[key] as {operator?:string;value?:string}|undefined)??{}; return <div key={key} className="playlist-text-rule"><label>{label}<input maxLength={256} value={rule.value??""} onChange={e=>update(key,{operator:rule.operator??"contains",value:e.target.value})} /></label><label>{label} comparison<select value={rule.operator??"contains"} onChange={e=>update(key,{value:rule.value??"",operator:e.target.value})}><option value="contains">Contains</option><option value="equals">Equals</option><option value="startsWith">Starts with</option><option value="doesNotContain">Does not contain</option></select></label></div>;})}
      <label>Genres (comma separated)<input value={genres.join(", ")} onChange={e => update("genres", e.target.value.split(",").map(v => v.trim()).filter(Boolean))} /></label>
      <label>Minimum rating<input type="number" min={0} max={5} step={request.view === "albums" ? 0.05 : 0.5} value={request.filters[ratingKey] == null ? "" : Number(request.filters[ratingKey])/ratingScale} onChange={e => update(ratingKey, e.target.value ? Math.round(Number(e.target.value)*ratingScale*10)/10 : null)} placeholder="Any" /></label>
      <label>Maximum rating<input type="number" min={0} max={5} step={request.view === "albums" ? 0.05 : 0.5} value={request.filters[maximumRatingKey] == null ? "" : Number(request.filters[maximumRatingKey])/ratingScale} onChange={e => update(maximumRatingKey, e.target.value ? Math.round(Number(e.target.value)*ratingScale*10)/10 : null)} placeholder="Any" /></label>
      <label>Original year from<input type="number" min={1} max={9999} value={String(request.filters.yearFrom ?? "")} onChange={e => update("yearFrom", e.target.value ? Number(e.target.value) : null)} /></label>
      <label>Original year to<input type="number" min={1} max={9999} value={String(request.filters.yearTo ?? "")} onChange={e => update("yearTo", e.target.value ? Number(e.target.value) : null)} /></label>
      <label>Sort<select value={request.sort.field} onChange={e => setRequest({ ...request, sort: { ...request.sort, field: e.target.value } })}>{Array.from(new Set([request.sort.field,"title","artist","album","year","trackRating","albumRating"])).map(field => <option key={field} value={field}>{field}</option>)}</select></label>
      <label>Direction<select value={request.sort.direction} onChange={e => setRequest({ ...request, sort: { ...request.sort, direction: e.target.value } })}><option value="asc">Ascending</option><option value="desc">Descending</option></select></label>
      <label>Song limit<input type="number" required min={1} max={10000} value={limit} onChange={e => setLimit(Number(e.target.value))} /></label>
      <label>Refresh<select value={policy} onChange={e => setPolicy(e.target.value as SmartPlaylistSettings["refreshPolicy"])}><option value="library">When the library changes / on open</option><option value="manual">Manual</option></select></label>
      <label><input type="checkbox" checked={Number(request.filters.lovedTracksMin ?? 0) > 0} onChange={e => update("lovedTracksMin", e.target.checked ? 1 : null)} />{request.view === "albums" ? "Albums with loved songs" : "Loved songs only"}</label>
    </fieldset>
    {error && <p role="alert">{error}</p>}
    <div className="saved-playlists__actions"><button disabled={busy || !name.trim()} type="submit">{busy ? "Saving…" : "Save Smart playlist"}</button><button disabled={busy} type="button" onClick={onCancel}>Cancel</button></div>
  </form>;
}
