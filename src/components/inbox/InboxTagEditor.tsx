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
  album: InboxAlbum;
  tracks: InboxTrack[];
  onApplied: () => void | Promise<void>;
}

function snapshotForTracks(album: InboxAlbum, tracks: InboxTrack[]): TagEditorSnapshot {
  return {
    tracks: tracks.map((track) => ({
      trackId: track.path,
      trackKey: track.path,
      revision: `${album.modifiedAtMs}:${track.path}`,
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

export function InboxTagEditor({ album, tracks, onApplied }: InboxTagEditorProps) {
  const currentRef = useRef({ album, tracks, onApplied });
  useEffect(() => {
    currentRef.current = { album, tracks, onApplied };
  }, [album, onApplied, tracks]);

  const loadSnapshot = useCallback(async () => {
    const current = currentRef.current;
    return snapshotForTracks(current.album, current.tracks);
  }, []);

  const saveSnapshot = useCallback(async (
    expected: TagEditorSnapshot,
    fields: EditableTagField[],
    values: EditableTagValues,
  ): Promise<ManualTagEditorSaveResult> => {
    const current = currentRef.current;
    const result = await applyInboxTags({
      albumPath: current.album.path,
      fields,
      tracks: current.tracks.map((track) => ({ path: track.path, values })),
      renameAfterApply: false,
    });
    await current.onApplied();
    const fieldLabel = `${fields.length} ${fields.length === 1 ? "field" : "fields"}`;
    const fileLabel = `${current.tracks.length} ${current.tracks.length === 1 ? "MP3" : "MP3s"}`;
    const changed = result.changedTracks === current.tracks.length
      ? ""
      : ` ${result.changedTracks} contained changes.`;
    return {
      state: updatedSnapshot(expected, fields, values),
      message: `Saved ${fieldLabel} directly to ${fileLabel}.${changed}`,
    };
  }, []);

  const singleTrack = tracks.length === 1 ? tracks[0] : null;
  return <ManualTagEditor
    kind={singleTrack ? "track" : "album"}
    label={singleTrack?.title ?? singleTrack?.fileName ?? album.album ?? album.folderName}
    loadSnapshot={loadSnapshot}
    saveSnapshot={saveSnapshot}
  />;
}
