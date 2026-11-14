import { AlertTriangle, LoaderCircle, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useAlbumCoverUrl } from "../../albumArtwork";
import { loadInboxSettings, type InboxAlbum } from "../../inbox";
import { loadLibraryAutoTagAlbum, type LibraryAutoTagResult } from "../../libraryAutoTag";
import { AlbumAutoTagger } from "./AlbumAutoTagger";

interface LibraryAutoTagDialogProps {
  albumId: string;
  albumTitle: string;
  onOpenSettings: () => void;
  onClose: () => void;
  onApplied: (message: string, result: LibraryAutoTagResult) => void | Promise<void>;
}

type Loaded = { album: InboxAlbum; discogsConfigured: boolean };

/** Opens the Inbox auto-tagger on a library album, read straight from its files. */
export function LibraryAutoTagDialog({ albumId, albumTitle, onOpenSettings, onClose, onApplied }: LibraryAutoTagDialogProps) {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [error, setError] = useState<string | null>(null);
  const coverUrl = useAlbumCoverUrl(albumId, 256);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([
      loadLibraryAutoTagAlbum(albumId),
      loadInboxSettings().then((settings) => settings.discogsConfigured).catch(() => false),
    ]).then(([album, discogsConfigured]) => {
      if (cancelled) return;
      if (album.tracks.length === 0) setError("This album has no MP3s Aurora can tag.");
      else setLoaded({ album, discogsConfigured });
    }).catch((nextError: unknown) => {
      if (!cancelled) setError(nextError instanceof Error ? nextError.message : String(nextError));
    });
    return () => { cancelled = true; };
  }, [albumId]);

  useEffect(() => {
    if (loaded) return undefined;
    const close = (event: KeyboardEvent) => { if (event.key === "Escape") onClose(); };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [loaded, onClose]);

  if (loaded) {
    return <AlbumAutoTagger
      albums={[loaded.album]}
      tracks={loaded.album.tracks}
      discogsConfigured={loaded.discogsConfigured}
      onOpenSettings={onOpenSettings}
      onClose={onClose}
      library={{ albumId, coverUrl }}
      onApplied={async (message, _path, result) => { if (result) await onApplied(message, result); }}
    />;
  }

  return <div className="modal-backdrop inbox-tagger-backdrop" role="presentation">
    <section className="inbox-tagger inbox-tagger--loading" role="dialog" aria-modal="true" aria-label={`Auto-tag ${albumTitle}`} aria-busy={!error}>
      <header>
        <div><p className="eyebrow">Library metadata</p><h2>Album Auto-Tagger</h2></div>
        <button type="button" aria-label="Close auto-tagger" onClick={onClose}><X /></button>
      </header>
      {error
        ? <p className="inbox-tagger__error" role="alert"><AlertTriangle />{error}</p>
        : <p className="inbox-tagger__status is-updating" role="status"><LoaderCircle className="is-spinning" aria-hidden="true" />Reading {albumTitle} from its MP3s…</p>}
    </section>
  </div>;
}
