# Scripting and plugins (potential feature)

*Status: proposal, not scheduled · Written 14 November 2026 · Not implemented in any release*

This document describes a possible scripting and plugin layer for Aurora: what it is, what it could unlock, how it differs from an MCP server, and how it could fit Aurora's existing safety rules. It is a decision aid, not a commitment. Nothing here changes current behavior.

## Contents

1. [Background](#background)
2. [What scripting and plugins mean](#what-scripting-and-plugins-mean)
3. [What it could unlock for Aurora](#what-it-could-unlock-for-aurora)
4. [How it differs from MCP](#how-it-differs-from-mcp)
5. [Should Aurora have it?](#should-aurora-have-it)
6. [Design principles](#design-principles)
7. [Architecture sketch](#architecture-sketch)
8. [Suggested phases](#suggested-phases)
9. [Risks and open questions](#risks-and-open-questions)
10. [Decision checklist](#decision-checklist)

---

## Background

This idea came out of comparing Aurora with other music managers (November 2026):

| App | Scripting / extensibility |
| --- | --- |
| **Helium Music Manager** 18.2 | Script engine in C# and JavaScript for batch tasks and *custom metadata sources*; plugins for external services, UI elements and customization. Also an MCP server (local only, secret token, approval before changes) and an in-app AI assistant. |
| **MediaMonkey** 2024 | Add-ons and skins; historically a scripting API and Winamp-style DSP/visualization plugins. |
| **Roon** 2.x | No user scripting; extensions only through the separate Roon API (mainly remote control and integrations). |

Helium's public material only names the script languages and purposes. It does not document the API, so the design below is Aurora's own and not a copy of Helium's.

Aurora currently has no scripting or plugin layer. The closest planned work is [roadmap Feature #20, Local read-only MCP server](improvement-roadmap.md) and Feature #7, natural-language search.

## What scripting and plugins mean

The same words describe three different layers. They can be built separately.

1. **Batch scripts.** Code you run on a selection: "for every track in these albums, change X". A programmable tag editor.
2. **Hooks.** Code the app calls at fixed moments: *after Inbox intake*, *on track played*, *before a tag save*, *after a rating*. The script reacts to events instead of being started by hand.
3. **Plugins / extension points.** Code that fills a slot the app provides:
   - a **metadata provider** (a new source the Album Auto-Tagger can search beside MusicBrainz and Discogs),
   - a **context-menu action**,
   - a **computed search field** or smart-playlist rule,
   - a **sidebar panel**.

"Custom metadata sources" in Helium is the third layer.

## What it could unlock for Aurora

### Metadata sources

- Soundtrack and score databases, Bandcamp, label sites, a Norwegian release database, or a personal spreadsheet. Each would appear in the Auto-Tagger next to MusicBrainz and Discogs and reuse the existing compare, reconcile and apply flow.
- Sources too niche or too personal to justify a built-in integration.

### Personal tag rules as reusable scripts

- "Soundtrack albums: put the composer in Album Artist."
- "Strip `(Remastered 2011)`-style suffixes from titles, but keep `(Live)`."
- "Normalize `feat.`, `ft.` and `featuring` to one spelling."
- Rules written once and run on any selection through the existing verified batch save.

### Automation through hooks

- **Intake:** extra readiness checks before an Inbox album becomes Ready (for example "every soundtrack must have a composer").
- **Playback:** send now-playing to Home Assistant, Discord or a custom scrobbler.
- **Ratings:** when an album becomes fully rated, add it to a playlist or log it.

### Computed fields

- New search fields or smart-playlist rules calculated by a script, for example `decade_gap:` (years between Original Year and Release Year) or "albums whose average rating is above the artist's average".

### Changes without a release

Every Aurora change today needs a SemVer bump, CI and a signed release build. A script is a file: edit it and run it immediately. This is the strongest argument for a personal app.

## How it differs from MCP

The difference is **who drives**.

- **Scripting / plugins:** your code runs *inside* Aurora. Aurora calls it at defined moments (a hook, a menu action, a metadata lookup) or you run it on a selection.
- **MCP:** an *outside* AI agent (Claude Code, Claude Desktop) drives Aurora. Aurora publishes a list of tools; the agent decides which to call during a conversation.

| | Scripting / plugins | MCP |
| --- | --- | --- |
| Who starts it | Aurora (hooks) or you (run script) | An AI agent, in a chat |
| Repeatable, deterministic | Yes: same input, same result | No: the model decides each time |
| Runs automatically and offline | Yes, no API cost | Needs an agent session |
| Adds new capabilities | Yes (providers, fields, actions) | No, only uses existing tools |
| Best for | Fixed rules, automation, integrations | One-off questions, exploration, "clean up these 40 albums" |

They work well together. Ask Claude through MCP to work out a cleanup rule, then save it as a script that runs on every intake from then on. Helium appears to ship both.

## Should Aurora have it?

The classic reason for plugins is letting *other developers* extend an app. That matters little here: Aurora is a personal app, and a missing feature can be built directly. What remains valuable:

- **Changes without a release** for small, personal rules.
- **Personal rules that do not belong in the product**, such as house style for soundtrack tagging.
- **Integrations** with home automation or other personal services.

Against it:

- Aurora's safety guarantees (below) are its main strength, and a scripting layer is the easiest way to weaken them.
- A script API becomes a contract that has to stay stable across Aurora releases.
- Many one-off batch tasks may be served well enough by MCP plus Claude.

**Current recommendation:** build the MCP server (roadmap #20) first. It reuses Aurora's existing bounded commands and will show which actions you actually repeat. Those repeated actions become the first scripts. Revisit this proposal after MCP has been used for a while.

## Design principles

Any scripting layer must keep the rules Aurora already follows:

1. **Scripts propose; Aurora writes.** A script never writes MP3 files, the Aurora state database or the catalog directly. It returns a list of proposed changes. Those go through the existing tag-editing pipeline: working copy, verification including the audio SHA-256, atomic replacement, journal and rollback. See [tag editing contract](tag-editing-contract.md).
2. **Review before apply by default.** Proposed changes appear in a review dialog with a diff, like the Auto-Tagger. Unattended apply is opt-in per script and limited to hooks the user enabled.
3. **The catalog stays read-only.** Scripts read through the same `query_only` catalog access Aurora uses. Music Library remains the only process that writes the catalog or moves files.
4. **Bounded payloads.** Script reads use the same paged, bounded queries as the UI (at most 100 rows per page, at most 200 queue tracks). A script cannot load the whole 1.1M-track library in one call.
5. **Sandboxed by default.** No file system, process or network access unless the script declares a permission and the user grants it. Network access is limited to declared hosts.
6. **Secrets stay in the credential vault.** Scripts ask for a named credential; they never see or store raw keys in their own files.
7. **Bounded runtime.** Per-run limits on time, memory and operations. A runaway script is stopped without affecting playback or the UI.
8. **Never on the audio path.** Scripts never run in the real-time audio callback or the PCM producer threads. Playback hooks are notified asynchronously after the fact.
9. **Visible and auditable.** Every run is logged with its proposals and outcome, like the Inbox History.

## Architecture sketch

### Language options

| Option | Notes | Fit |
| --- | --- | --- |
| [Rhai](https://rhai.rs) | Pure Rust, sandboxed by default, operation and memory limits built in, simple syntax. Small ecosystem. | **Best first choice** for batch scripts and hooks. |
| JavaScript via `rquickjs` (QuickJS) | Familiar language, good for metadata providers that parse JSON/HTML. Needs explicit sandboxing. | Good second option, especially for providers. |
| Lua via `mlua` | Fast, proven in games and editors. Sandboxing must be configured carefully. | Viable. |
| WebAssembly via Extism or `wasmtime` | Strong isolation, any source language, versioned host API. Heavier to build and maintain. | Only if real plugins with UI or third-party authors become a goal. |

Avoid C#/.NET: it would add a second runtime to a Rust/Tauri app for little gain.

### Host API (first version)

Read-only:

- `search(query, page)`: same query language as the top bar, returns bounded pages.
- `album(id)`, `tracks(album_id)`, `track(path)`: catalog reads plus Aurora's pending overlays.
- `history(range, page)`: listening history reads.

Proposals:

- `propose_tag_change(track, field, old, new)`: queued for review; `old` must match or the proposal is rejected as stale.
- `propose_playlist_add(playlist, tracks)`: goes through the existing Music Library playlist bridge.

Providers:

- `provider.search(artist, album)` → candidate releases.
- `provider.release(id)` → tracks, dates, label, artwork URL, in the same shape the Auto-Tagger already uses for MusicBrainz and Discogs.

Hooks (asynchronous):

- `on_intake_ready(album)` → readiness issues to show in Inbox.
- `on_track_played(track, session)`, `on_rating_changed(track, old, new)`.

### Script files

- Stored under `%APPDATA%\com.soundtrackgeek.aurora\scripts\`, one folder per script, each with a small manifest (name, version, kind, declared permissions, required host API version) and the script source.
- Managed in **Settings → Scripts**: list, enable or disable, grant permissions, run, view log.
- Optionally synced between devices like other Aurora state, but disabled by default on a new device until permissions are granted again.

### Illustrative example (Rhai)

This shows the intended shape only; the API names are not final.

```rust
// Normalize "feat." spellings in track titles of the selected albums.
fn run(selection) {
    for track in selection.tracks() {
        let title = track.title;
        let fixed = title.replace(" ft. ", " feat. ").replace(" featuring ", " feat. ");
        if fixed != title {
            propose_tag_change(track, "title", title, fixed);
        }
    }
}
```

Running it opens the usual review dialog with every proposed change; nothing is written until **Apply**.

## Suggested phases

| Phase | Scope | Depends on |
| --- | --- | --- |
| 0 | MCP server, read-only (roadmap #20) | Existing bounded commands |
| 1 | MCP proposal tools with review dialog (Helium-style approval) | Phase 0, tag pipeline |
| 2 | Batch scripts in Rhai: read API + `propose_tag_change`, run from a selection's context menu | Phase 1's review dialog |
| 3 | Metadata provider slot in the Album Auto-Tagger | Phase 2 runtime, network permission model |
| 4 | Hooks (intake readiness, played, rating changed) | Phase 2, native event bus |
| 5 | Optional: WebAssembly plugins with UI panels | Only if phases 2–4 prove their worth |

Phases 1 and 2 share the same review-before-apply component, so building MCP proposals first reduces the cost of scripting later.

## Risks and open questions

- **API stability.** Once scripts exist, renaming catalog fields or changing search semantics can break them. A versioned host API and a manifest field for the required version are needed from the start.
- **Unattended writes.** Hooks that apply changes without review are convenient but conflict with the review-first principle. Should they be allowed at all, or only for non-tag actions such as playlist adds?
- **Performance.** A script that proposes changes for 50,000 tracks needs batching, progress and cancel, like Inbox intake.
- **Network Mode on the Mac.** On the Mac, tag edits go through Tonehavn. Proposals from scripts would need to follow that path, or scripts would be Windows-only at first.
- **Music Library overlap.** Some automation (intake checks, playlist rules) could live in Music Library instead. Where should the scripting runtime live, or should both apps share one?
- **Is it needed?** If MCP plus Claude already covers the repeated tasks, phases 2–5 may never be worth building.

## Decision checklist

Before scheduling any phase beyond MCP, answer:

- [ ] Which concrete tasks have been repeated through MCP or by hand at least a few times?
- [ ] Which of them need to run automatically (hooks) rather than on demand?
- [ ] Is there at least one metadata source worth a provider plugin?
- [ ] Is Rhai acceptable as the only language for the first version?
- [ ] Are unattended writes allowed, and for which actions?
- [ ] Should scripts work in Mac Network Mode from day one?
