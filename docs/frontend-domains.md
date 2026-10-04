# Frontend domain architecture

Aurora's frontend separates the application shell from domain state and destination rendering. This implements [roadmap improvement 1](improvement-roadmap.md#1-split-apptsx-into-domain-modules). It does not change the native catalog, playback, or file-write contracts.

## Ownership

| Module | Responsibility |
| --- | --- |
| `src/App.tsx` | Shell composition and explicit handoffs between domains: navigation, playback, tag projections, and intake results. |
| `src/app/appStore.ts` | App-scoped shared destination, selected track, and catalog invalidation revision. Stable actions support functional updates. |
| `src/app/AppStoreProvider.tsx` | Store lifetime and React integration. Each app mount has its own store. |
| `src/app/explorer/` | Explorer filters, loaded windows, keyset cursors, query loading, file refresh, and stale-request guards. |
| `src/app/navigation/` | Retained view snapshots, Back history, workspace checkpoints, and readiness-gated scroll restoration. |
| `src/app/inspector/` | Track and album selection, detail loading, playback-follow behavior, and inspector metadata. |
| `src/app/artist/` | Artist inspector requests and independently arriving catalog and MusicBrainz results. |
| `src/app/catalog/` | Consistent catalog revision refresh, monotonic tag projections, reconciliation, and pending library-sync retries. |
| `src/app/tags/` | MP3 edits, optimistic updates and rollback, shortcut results, and projection of verified changes into domain caches. |
| `src/app/domains/` | History, Genres, Publishers, Years, Ratings, Observatory, and Playlists state, requests, and actions. |
| `src/app/settings/` | Settings loading/saving and Laptop Mode status. |
| `src/app/routes/` | Lazy destination containers and inspectors. Runtime imports of heavy destination components belong here. |
| `src/app/components/` | Shell-adjacent presentation components and lazy settings, tag-editor, and album-operation panels. |

The store's catalog revision is a local invalidation counter. It also advances after successful tag synchronization when the native import revision has not changed. `useCatalogRevision` separately owns the opaque native revision and verifies that playback and library snapshots refer to the same catalog before applying them.

## Extending a domain

Keep state and async guards with the domain that owns the request. Hooks accept typed inputs and expose explicit actions and state; shared selections are passed through typed ports. Cross-domain actions stay visible in shell composition instead of importing one domain's mutable internals into another. Keep callback identities stable when a polling effect depends on them.

Use `useAppSlice` for a shared field. Do not subscribe a destination to the entire store or introduce a process-wide singleton. Store selectors return existing field values so unrelated updates do not re-render a consumer. Page-specific filters, results, cursors, and errors remain in their domain.

Add new runtime page imports through a lazy route boundary. Type-only imports are safe outside those boundaries. Keep `RememberedPage` outside each route: it mounts only after the first visit, freezes hidden props, retains local state, and pauses inactive effects. Avoid eager imports of inspectors from a heavy page module, which would pull that page into the entry bundle again.

## Behavior that must survive a refactor

- Preserve selected tracks and albums, expanded details, filter/sort state, loaded pages, and destination-specific Back history.
- Restore scroll only after the relevant results, details, and lazy content are ready; user input can cancel restoration.
- Invalidate obsolete requests before navigation or selection changes. A late catalog or file response must not overwrite a newer view or tag edit.
- Apply only consistent catalog/playback snapshots. Preserve per-track projection ordering and serialized pending-sync retries.
- Keep existing bounded queue, request, and result limits, and retain the file/catalog authority boundary.

Run the focused domain tests for the boundary being changed, then the existing `App.*.test.tsx` integration tests and CI commands. The store and lazy-route tests verify subscription isolation, separate app instances, first-visit loading, retention, and invalidation on return. Browser preview verifies navigation and rendering; native device playback and installed-app lifecycle behavior require native verification.
