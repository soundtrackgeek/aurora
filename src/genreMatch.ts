export const MAX_GENRE_SUGGESTIONS = 40;

function fold(text: string): string {
  return text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

function words(text: string): string[] {
  return fold(text).split(/[\s\-/&+,.()]+/).filter(Boolean);
}

/** Each query word must start a word of the genre, in order ("alt dan" → "Alternative Dance"). */
function wordPrefixRank(queryWords: string[], genreWords: string[]): number | null {
  let next = 0;
  let first = -1;
  for (const queryWord of queryWords) {
    while (next < genreWords.length && !genreWords[next].startsWith(queryWord)) next += 1;
    if (next >= genreWords.length) return null;
    if (first < 0) first = next;
    next += 1;
  }
  return first === 0 ? 2 : 3;
}

export function matchGenreSuggestions(query: string, genres: readonly string[], limit = MAX_GENRE_SUGGESTIONS): string[] {
  const folded = fold(query.trim());
  if (!folded) return [];
  const queryWords = words(folded);
  const ranked: Array<{ genre: string; rank: number }> = [];
  for (const genre of genres) {
    const name = fold(genre);
    let rank: number | null = null;
    if (name === folded) rank = 0;
    else if (name.startsWith(folded)) rank = 1;
    else if (queryWords.length) rank = wordPrefixRank(queryWords, words(genre));
    if (rank === null && name.includes(folded)) rank = 4;
    if (rank !== null) ranked.push({ genre, rank });
  }
  ranked.sort((left, right) => left.rank - right.rank
    || left.genre.length - right.genre.length
    || left.genre.localeCompare(right.genre));
  return ranked.slice(0, limit).map(({ genre }) => genre);
}
