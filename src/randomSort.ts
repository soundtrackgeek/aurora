/** Stable across pages, refreshes, and saved views; mirrors the native catalog rank. */
export function randomSortRank(id: string, seed = 0): number {
  let hash = (2166136261 ^ seed) >>> 0;
  for (const byte of new TextEncoder().encode(id)) hash = Math.imul(hash ^ byte, 16777619) >>> 0;
  hash = Math.imul(hash ^ (hash >>> 16), 0x85ebca6b) >>> 0;
  hash = Math.imul(hash ^ (hash >>> 13), 0xc2b2ae35) >>> 0;
  return (hash ^ (hash >>> 16)) >>> 0;
}

export function nextRandomSortSeed(previous?: number): number {
  const seed = crypto.getRandomValues(new Uint32Array(1))[0];
  return seed === previous ? (seed + 1) >>> 0 : seed;
}
