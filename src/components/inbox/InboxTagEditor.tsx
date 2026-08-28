import { useCallback, useEffect, useRef } from "react";
import { applyInboxTags, type InboxAlbum, type InboxTrack } from "../../inbox";
import type {
  EditableTagField,
  EditableTagValues,
  TagEditorSnapshot,
} from "../../tags";
import {
  ManualTagEditor,
  type ManualTagEditorSaveResult,
} from "../TagEditor";

interface InboxTagEditorProps {
  albums: InboxAlbum[];
  tracks: InboxTrack[];
  onApplied: () => void | Promise<void>;
}

function snapshotForTracks(albums: InboxAlbum[], tracks: InboxTrack[]): TagEditorSnapshot {
  const revisions = new Map(albums.flatMap((album) => album.tracks.map((track) => [track.path, album.modifiedAtMs] as const)));
  return {
    tracks: tracks.map((track) => ({
      trackId: track.path,
      trackKey: track.path,
      revision: `${revisions.get(track.path) ?? 0}:${track.path}`,
      values: {
        albumArtist: track.albumArtist,
        artist: track.artist,
        album: track.album,
        title: track.title,
        genre: track.genre,
        publisher: track.publisher,
        rating: track.rating,
        year: track.year,
        releaseYear: track.releaseYear,
        trackNumber: track.trackNumber,
        trackTotal: track.trackTotal,
        discNumber: track.discNumber,
        discTotal: track.discTotal,
      },
    })),
  };
}

function updatedSnapshot(
  snapshot: TagEditorSnapshot,
  fields: EditableTagField[],
  values: EditableTagValues,
): TagEditorSnapshot {
  return {
    tracks: snapshot.tracks.map((track) => ({
      ...track,
      values: fields.reduce<EditableTagValues>((next, field) => ({
        ...next,
        [field]: values[field],
      }), { ...track.values }),
    })),
  };
}

export function InboxTagEditor({ albums, tracks, onApplied }: InboxTagEditorProps) {
  const currentRef = useRef({ albums, tracks, onApplied });
  useEffect(() => {
    currentRef.current = { albums, tracks, onApplied };
  }, [albums, onApplied, tracks]);

  const loadSnapshot = useCallback(async () => {
    const current = currentRef.current;
    return snapshotForTracks(current.albums, current.tracks);
  }, []);

  const saveSnapshot = useCallback(async (
    expected: TagEditorSnapshot,
    fields: EditableTagField[],
    values: EditableTagValues,
  ): Promise<ManualTagEditorSaveResult> => {
    const current = currentRef.current;
    const selectedPaths = new Set(current.tracks.map((track) => track.path));
    const batches = current.albums.map((album) => ({
      album,
      tracks: album.tracks.filter((track) => selectedPaths.has(track.path)),
    })).filter((batch) => batch.tracks.length > 0);
    const results = await Promise.all(batches.map(({ album, tracks: albumTracks }) => applyInboxTags({
      albumPath: album.path,
      fields,
      tracks: albumTracks.map((track) => ({ path: track.path, values })),
      renameAfterApply: false,
    })));
    await current.onApplied();
    const fieldLabel = `${fields.length} ${fields.length === 1 ? "field" : "fields"}`;
    const fileLabel = `${current.tracks.length} ${current.tracks.length === 1 ? "MP3" : "MP3s"}`;
    const changedTrackCount = results.reduce((total, result) => total + result.changedTracks, 0);
    const changed = changedTrackCount === current.tracks.length
      ? ""
      : ` ${changedTrackCount} contained changes.`;
    return {
      state: updatedSnapshot(expected, fields, values),
      message: `Saved ${fieldLabel} directly to ${fileLabel}.${changed}`,
    };
  }, []);

  const singleTrack = tracks.length === 1 ? tracks[0] : null;
  const label = singleTrack?.title ?? singleTrack?.fileName
    ?? (albums.length === 1 ? albums[0]?.album ?? albums[0]?.folderName : `${albums.length} albums`)
    ?? "Inbox selection";
  return <ManualTagEditor
    kind={singleTrack ? "track" : "album"}
    label={label}
    loadSnapshot={loadSnapshot}
    saveSnapshot={saveSnapshot}
  />;
}
