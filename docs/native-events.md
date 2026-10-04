# Native background events

Aurora 0.28.20 implements roadmap improvement 2. Background work continues while the WebView is hidden or minimized. Each native job runs on its own named thread; jobs never overlap themselves. A slow share or Music Library bridge cannot delay the playback or catalog workers.

| Job | Native interval | Event |
| --- | --- | --- |
| State snapshot publication/status | 5 s, preserving the service's 60 s publication throttle | `sync://status` |
| Completed catalog import revision | 5 s | `catalog://revision` |
| Pending tag reconciliation | 5 s, at most 100 overlays per pass | `tags://reconciled` |
| Durable library retries | 5 s, at most one eligible folder per pass | `library-sync://status` |
| Playback runtime maintenance | 250 ms | `playback://state` on transitions only |
| History publication and local/peer revision detection | 5 s, preserving history publication throttling | `history://revision` |

Status and history revision events are suppressed while their payloads are unchanged. Catalog revisions repeat until React acknowledges a successful refresh; a failed refresh therefore recovers on the next native pass, while acknowledged revisions generate no more events. Playback events include the bounded queue and state on track, transport, seek, queue, gain/device, and error changes. Normal playhead movement emits no stream; React keeps the existing 250 ms local clock. Native playback maintenance drives automatic advancement, gapless preparation, history observation, and ordered persistence even without frontend requests. A 15-second frontend heartbeat repairs a missed playback event; browser preview keeps its simulated two-second playback clock.

Listeners register before their initial read, and cleanup releases even registrations that finish after unmount. Catalog refresh still verifies matching catalog, rebound playback, and library revisions. Tag reconciliation and library status retain the existing projection tokens. Every playback snapshot receives a sequence under the playback lock, including command responses and native shortcut/media actions; React rejects older snapshots. Laptop Mode status reads cached sync results and includes album-order revisions. Reading status does not publish a snapshot. Initial library status reads inspect durable queue counts without running the bridge; only native retries and explicit retry commands run it. Partial retry completions emit status even when the remaining counts are unchanged.

Native retries reuse the existing durable queue, transient backoff, terminal blocking rules, bridge arbitration, and read-only Network Mode boundaries. No worker writes the shared catalog directly or replays intake transfers. Shutdown first drains local playback/history persistence; failed drains leave schedules running so storage can recover. Successful process exit signals every native schedule to stop. Window close retains the final forced state publication.

Automatic update checks run at startup, every six hours after success, and on focus if the last attempt was at least an hour ago. Failures retry after 5, 10, 20, 40, 80, 160, then 320 minutes, capped at six hours. Focus respects a pending failure backoff. Manual checks bypass timing limits; check/install concurrency guards still apply. Browser preview and development builds make no update requests.

Inbox's current scan/focus refresh is unchanged here. Filesystem watching and safer conversion belong to roadmap improvement 17.
