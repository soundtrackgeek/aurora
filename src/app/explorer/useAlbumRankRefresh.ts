import { useEffect, useLayoutEffect, useRef, type Dispatch, type SetStateAction } from "react";
import { loadAlbumYearRanks, type AlbumSummary } from "../../library";

/** Wait for the rating save behind a Score change before asking for fresh ranks. */
export const ALBUM_RANK_REFRESH_DELAY_MS = 900;

/**
 * Keeps each visible album's Rank current. When an album's Score changes, every
 * visible album of the same original year is re-ranked, since one album moving can
 * push its neighbours up or down.
 */
export function useAlbumRankRefresh(
  albums: readonly AlbumSummary[],
  setAlbums: Dispatch<SetStateAction<AlbumSummary[]>>,
) {
  const scoresRef = useRef(new Map<string, number | null>());
  const pendingYearsRef = useRef(new Set<number>());
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const albumsRef = useRef(albums);
  useLayoutEffect(() => {
    albumsRef.current = albums;
  }, [albums]);

  useEffect(() => {
    const previous = scoresRef.current;
    const next = new Map<string, number | null>();
    for (const album of albums) {
      next.set(album.id, album.albumScore);
      if (previous.has(album.id) && previous.get(album.id) !== album.albumScore && album.originalYear != null) {
        pendingYearsRef.current.add(album.originalYear);
      }
    }
    scoresRef.current = next;
    if (pendingYearsRef.current.size === 0) return;
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      timerRef.current = null;
      const years = new Set(pendingYearsRef.current);
      pendingYearsRef.current.clear();
      const ids = albumsRef.current
        .filter((album) => album.originalYear != null && years.has(album.originalYear))
        .map((album) => album.id);
      if (ids.length === 0) return;
      loadAlbumYearRanks(ids).then((ranks) => {
        setAlbums((current) => {
          let changed = false;
          const updated = current.map((album) => {
            if (!(album.id in ranks) || (album.yearRank ?? null) === ranks[album.id]) return album;
            changed = true;
            return { ...album, yearRank: ranks[album.id] };
          });
          return changed ? updated : current;
        });
      }).catch((error: unknown) => {
        console.warn("Aurora could not refresh album ranks", error);
      });
    }, ALBUM_RANK_REFRESH_DELAY_MS);
  }, [albums, setAlbums]);

  useEffect(() => () => {
    if (timerRef.current) clearTimeout(timerRef.current);
  }, []);
}
