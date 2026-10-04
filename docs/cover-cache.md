# Cover protocol and disk cache

Aurora 0.28.23 implements improvement-roadmap item 3. `aurora-cover` uses Tauri's asynchronous responder; its callback only dispatches owned requests to a fixed pool of three workers. Catalog lookups, path validation, cache I/O, image decoding, resizing, and WebP encoding happen on those workers. The pool accepts at most 256 pending requests. Saturation returns HTTP 503 with `Cache-Control: no-store`, without blocking resource handling or starting extra workers. Unexpected request panics return 500 and leave the worker available.

## Disk budget and ordering

The device-local application cache has one **1 GiB (1,073,741,824 bytes)** budget for generated WebP files across these folders:

- `covers`: indexed album-cover archive thumbnails.
- `embedded-album-covers`: library albums using embedded MP3 artwork.
- `inbox-covers`: embedded artwork in validated Inbox tracks.
- `selected-covers`: preview thumbnails for registered cover-picker selections.

A startup worker enumerates existing thumbnail files and evicts the oldest until the combined size fits. Existing modification times provide the initial ordering; every disk-cache hit updates the thumbnail's modification time, so later starts retain access ordering even where filesystem access-time updates are disabled. An in-memory index orders subsequent evictions without rescanning directories on every request. All cache reads, eviction, and atomic publication share a lock; image decoding takes place outside it, allowing three simultaneous decodes.

Every new thumbnail reserves space by evicting least recently used entries before staging and renaming its output. Fingerprint names retain album/source identity, source length and modification time, and requested size, so a changed source produces a new entry. Older versions eventually age out under the same budget. Concurrent decodes of the same fingerprint reuse an already published entry.

Failed deletions remain counted. Other old entries are tried, and caching is skipped if enough space cannot be reclaimed. A cache already above budget because files cannot be deleted may remain above budget until those files become removable; new writes do not add to it. Cache I/O failure never hides an otherwise successfully decoded cover.

Cleanup recognizes only generated fingerprint/size `.webp` names and Aurora's `pid-sequence.tmp` staging names directly inside the four folders. It does not recurse or follow linked directories/files (including Windows junctions), and preserves unrelated files and caches. Original archive images, selected files, and music tracks are never eviction targets. This cache is derived local data and is not synchronized.

## Existing request contract

Album ID resolution, Inbox-path validation, selected-cover tokens, and embedded-cover fallback retain their existing authority boundaries. Thumbnail requests accept only 64, 128, 256, or 512 pixels. Source images remain limited to 32 MiB and 100 million pixels; the pixel limit is checked before thumbnail decoding. MP3 source size is separate from its embedded-picture size. Successful responses remain `image/webp` with immutable HTTP caching; malformed requests return 400 and unavailable covers return 404, both with `no-store`.

## Verification

Rust regressions use explicit worker gates and small temporary caches to verify parallel processing, bounded queue overflow, one cross-directory budget, cleanup boundaries, access ordering after restart, concurrent writes, and successful embedded artwork when caching fails. Existing thumbnail-size, embedded-fallback, and changed-source tests remain in place. Run the CI lint, frontend tests/build, Rust formatting, Clippy, and serialized Rust tests before pushing.

The browser preview does not exercise a native custom protocol. For native smoke testing, start Aurora with a cold cache, scroll Albums rapidly, open an album and an Inbox cover, then reopen the app with a warm cache. Check for usable artwork and responsive navigation. Unit coverage verifies scheduling and eviction; it does not measure installed WebView rendering or macOS behavior.
