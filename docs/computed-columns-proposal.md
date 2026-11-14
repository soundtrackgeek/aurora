# Computed columns (proposal)

*Status: proposal, not implemented · written 14 November 2026 against Aurora 0.57.4*

A computed column is a Songs column whose value comes from a short formula over
fields Aurora already knows, for example `ryear - year`. You create, show, hide,
reorder, resize and sort it like any built-in column. It is saved with your
column layout and with saved views.

The idea comes from JRiver Media Center, whose expression language drives
columns, grouping, display titles, rename rules and smartlists. Aurora already
has a mature search language (see [search.md](search.md)). Computed columns
would add a second, smaller language that **calculates** values instead of
**filtering** rows.

---

## 1. Why

### Problems it solves

- **Every new column is a release today.** Songs columns are a fixed union in
  `src/components/explorer/trackColumns.ts` (`TrackColumnId`). Each sortable
  column has a hand-written `TrackSort` variant and `SortDefinition` in
  `src-tauri/src/explorer.rs`. Plays (0.50.2), Charts and the 💿 singles badge
  each needed code, a version bump, CI and an install.
- **The interesting questions sit between fields.** Aurora stores two year
  fields (Original Year and Release Year), chart peaks from several countries,
  half-star ratings, play counts from every device, official singles and artist
  lifespans. Search can filter on each one, but it can't sort by how they
  relate: "how much later was this reissued", "how far above its album average
  is this track", "what was the artist's age when this came out".
- **Review lists.** Many housekeeping lists ("top-10 hit, still unrated",
  "loved but rated under 3★") are really a sortable flag column.

### What you gain

1. **Ad-hoc questions without a release.** Type a formula, sort, look, delete
   it. No code change.
2. **Sorting by relationships between fields.** This is something the search
   language can't do.
3. **One evaluator to reuse later.** The same formulas can later become search
   filters, smart playlist rules, grouping keys and rename templates (see §9).
4. **Built-in presets** so the feature is useful before anyone writes a formula.

### What it is not

- Not a scripting or plugin system. There are no loops, no writes to files and
  no network access.
- Not a way to change data. Computed values are never written to MP3s, the
  catalog or Music Library.
- Not a replacement for the search language. Filtering stays in search, and
  computed columns calculate and sort.

---

## 2. What it looks like

### Creating a column

The Songs **Columns** menu gets a **Computed** section with **+ New column**.
The editor is a small dialog:

```
┌ New computed column ───────────────────────────────────────┐
│ Name      [ Reissue gap                         ]           │
│ Formula   [ ryear - year                        ]           │
│           ✓ Valid · number · sortable                       │
│ Format    (•) Number  ( ) Text  ( ) Duration  ( ) Stars     │
│           ( ) Days ago  ( ) Flag                            │
│ Preview   Go Your Own Way · Rumours (2004)      → 27        │
│           Billie Jean · Thriller                → 0         │
│           Hounds of Love · Hounds of Love       → —         │
│                                     [Cancel]  [Add column]  │
└─────────────────────────────────────────────────────────────┘
```

- The formula is checked as you type, with the same kind of inline errors
  search already has ("Unknown field `ryeer`. Did you mean `ryear`?").
- **Preview** runs the formula on the first rows of the current page, so you
  see real results before saving.
- **Sortable** is shown, not chosen. It comes from which fields and functions
  the formula uses (see §5). A display-only formula still works as a column,
  but its header doesn't sort.
- **Presets** (§4) are listed above the button. One click adds them.

### In the table

Computed columns look like built-in ones. To tell them apart, the header shows a
small `ƒ` mark, and the header tooltip shows the formula. Empty results show as
`—`, the same as Plays.

---

## 3. The formula language

Small, typed and boring on purpose. Every formula is one expression.

### Fields (Songs, first version)

| Name | Type | Source |
| --- | --- | --- |
| `title`, `artist`, `aartist`, `album`, `genre`, `publisher` | text | catalog track/album columns |
| `year` | number? | Original Year (`t.year`) |
| `ryear` | number? | Release Year (`t.release_year`) |
| `track`, `disc` | number? | track/disc number |
| `duration` | number? (seconds) | `t.time_seconds` |
| `rating` | number? (0.5–5) | the existing `TRACK_RATING_EXPRESSION` ÷ 20 (catalog value; see §9 for pending overlays) |
| `love` | number (1 loved, 0 neutral, −1 banned) | same mapping as `TrackSort::Love*` |
| `plays` | number | registered plays (`temp.aurora_registered_track_plays`) |
| `single` | number (0/1) | official single flag (Music Library 0.210.0+) |
| `chart.bb`, `chart.uk`, `chart.vg`, `chart.ti`, `chart.nt` | number? | Music Library's materialized track chart ranks |
| `chart.best` | number? | smallest of the available chart ranks |
| `born`, `founded`, `died`, `dissolved` | number? (year) | album artist lifespan record (as `born:` search) |
| `album.mean_rating` | number? | mean of the album's rated tracks |
| `album.tracks`, `album.rated` | number | album track count / rated-track count |

`number?` means the value may be missing (`none`). There is no `0` that means
"unknown". Missing values carry through calculations (§3.3).

Later versions may add history fields (`last_played`, `first_played`) and sonic
fields (`brightness`, `level`). These are **display-only** in the first version
(see §5).

### Operators and functions

```
+  -  *  /           arithmetic (division by zero → none)
=  !=  <  <=  >  >=  comparison → boolean
and  or  not         boolean logic
if(cond, a, b)       conditional; both branches must have the same type
min(a, b, …)         smallest non-missing value; none if all are missing
max(a, b, …)         largest non-missing value
round(x), round(x, n)
abs(x)
coalesce(a, b, …)    first non-missing value
days_since(year)     whole days since 1 January of that year (number)
lower(s), upper(s), len(s)
"…{field}…"          text template, e.g. "{aartist} — {album} ({year})"
```

Literals: integers, decimals, `"quoted text"`, `none`, `true`, `false`.

Comments, variables, user-defined functions, regular expressions and loops are
**out of scope**.

### Rules for missing values

- Arithmetic with `none` gives `none`. Comparisons with `none` give `false`.
  `coalesce` is the explicit way to supply a default.
- `min`/`max` skip `none`, so `chart.best` works for tracks that only charted in
  one country.
- When sorting, `none` always goes **last**, whatever the direction. This
  matches how Aurora already treats missing years.

### Types

Every formula has one result type, worked out when it is saved: number, text,
boolean or duration. Type errors are reported when you save, never in the
middle of a sort. `if(rating > 4, "★", 3)` is rejected. The **Format** choice
must suit the type (a text formula can't use the Stars format).

---

## 4. Presets

These ship with the feature and can be edited after they are added.

| Preset | Formula | Format |
| --- | --- | --- |
| Reissue gap | `ryear - year` | Number |
| Age at release | `year - born` | Number |
| Best chart peak | `chart.best` | Number |
| Above album average | `round(rating - album.mean_rating, 1)` | Number (signed) |
| Hit, still unrated | `if(chart.best <= 10 and rating = none, "⚠", "")` | Flag |
| Loved but rated low | `if(love = 1 and rating < 3, "⚠", "")` | Flag |
| Album progress | `round(album.rated * 100 / album.tracks)` | Number (%) |
| Label | `"{aartist} — {album} ({year})"` | Text |

---

## 5. How it runs

The constraint that shapes everything: **Songs pages over 1.1M tracks with
keyset cursors and at most 100 rows per page.** A computed column has to fit
that model.

### Two tiers

| | Sortable formula | Display-only formula |
| --- | --- | --- |
| Allowed fields | catalog columns, rating, love, plays, chart ranks, singles, lifespans, album aggregates | anything in tier 1, plus later history and sonic fields |
| Evaluated by | **SQLite**, as a SQL expression in the page query | **Rust**, for the ≤100 rows of the current page |
| Sort / paginate | yes, through a keyset cursor | no (header doesn't sort) |
| Cost | one more expression in an existing query | at most 100 small evaluations per page |

The compiler decides the tier from the formula itself. A formula is sortable
only when **every** field and function in it has a SQL translation.

### Compiling to SQL

The formula is parsed into a small typed syntax tree in Rust (a new module, for
example `src-tauri/src/computed_columns.rs`). Each node translates to a
**bound-parameter** SQL fragment that uses the existing expressions:

| Formula | SQL fragment |
| --- | --- |
| `ryear` | `t.release_year` |
| `rating` | `(TRACK_RATING_EXPRESSION) / 20.0` |
| `plays` | `COALESCE(rp.plays, 0)` |
| `love` | the `CASE … 'L' THEN 2 …` expression from `TrackSort::Love*`, shifted to −1/0/1 |
| `a - b` | `(a) - (b)` (SQLite already carries `NULL` through) |
| `min(a, b)` | `CASE` chain that skips `NULL` (SQLite's own `min()` returns `NULL` when any argument is `NULL`) |
| `"x"` literal | `?N` bound parameter, **never** spliced into the SQL text |

User text never becomes SQL text. Literals are always bound parameters. Field
names come from a fixed list. The result is just another `SortDefinition`: a
generated `expression`, a `cursor_tag` like `computed:<hash of formula>`, and a
`CursorKind` derived from the result type. Keyset pagination then works
unchanged, with the track id as the tiebreaker as it is today.

Album aggregates (`album.mean_rating`, `album.rated`) are computed in one
`GROUP BY album_id` subquery joined into the page query. They are only joined
when a formula uses them.

### Wiring

- `TrackPageRequest` gets an optional `computed: Vec<ComputedColumnRequest>`
  (formula text, id) and an optional `computed_sort: { id, descending }`. The
  backend parses and validates every formula itself. It never trusts a
  front-end translation.
- `TrackSummary` gets `computed: Vec<ComputedValue>` (id → number/text/none),
  filled only for requested columns.
- Limits keep payloads bounded: at most **8** computed columns per request,
  **256** characters per formula, a syntax tree depth of **16**, and **32**
  nodes. Formulas over a limit are rejected with a clear message.

### Performance budget

- **Display-only columns:** at most 100 evaluations per page. Negligible.
- **Sorting by a computed column:** SQLite has to evaluate the expression for
  every row that matches the current search. There is no index on a formula, so
  the first page of an unfiltered Songs view means a full scan. That is the same
  cost class as **Plays · most** and **Random**, which already sort on
  computed/joined values over the full catalog.
- **Acceptance check:** with a search filter, the first page of a computed sort
  arrives as fast as a Plays sort on the same filter. Unfiltered, it stays
  within 2× of a Plays sort on the full catalog. If that fails, sortable
  formulas are only offered together with a search filter.

---

## 6. Storage

- **Definitions** (id, name, formula, format) live in Aurora's writable state
  database (`StateStore`), in a new `computed_columns` table. They don't live in
  `localStorage`, so they survive a WebView reset and can later sync with the
  rest of Aurora's state.
- **Layout:** `TrackColumnLayout` (`aurora:songs-columns:v1`) extends its
  `order`, `hidden` and `widths` to accept `computed:<id>` entries next to the
  built-in `TrackColumnId`s. Existing layouts migrate with all computed columns
  hidden.
- **Saved views:** `SavedView.filters` already holds arbitrary JSON. Views that
  sort by or show a computed column store its **formula text**, not only its id,
  so a view still works if the definition is deleted.
- The catalog and MP3s are never touched. Nothing new crosses the Music Library
  bridge.

---

## 7. Phased plan

| Phase | Scope | Size |
| --- | --- | --- |
| **1 · Display + sort in Songs** | Parser, type checker, SQL compiler, tier 1 fields, presets, Columns menu editor with preview, `StateStore` table, layout and saved-view storage | Medium |
| **2 · Display-only extras** | History fields (`last_played`, `first_played`, `days_since_played`), sonic fields, Rust evaluator for display-only formulas | Small–medium |
| **3 · Reuse in search** | `calc:"Reissue gap">=20` in the search language, compiled through the same SQL path, so computed columns work in saved views and smart playlist rules | Medium |
| **4 · Other destinations** | Albums computed fields (album aggregates as values), grouping by a computed value | Medium |
| **5 · Rename templates** | Text formulas as Inbox rename templates, behind the existing rename preview and verified rename pipeline | Medium |

Phase 1 already delivers the main gain. Each later phase is optional.

---

## 8. Testing

- **Parser and type checker:** a table of valid and invalid formulas with exact
  error messages and positions. Every function gets tested with `none` inputs.
- **SQL compiler:** each node's SQL fragment, checked against SQLite on a small
  fixture catalog (the existing `explorer.rs` fixtures). Literals must appear
  only as bound parameters. A test feeds `"'; DROP TABLE tracks; --"` as a
  literal and checks that it is returned as text.
- **Pagination:** sorting by a computed column across page boundaries with
  ties, `none` values and both directions returns every row exactly once. This
  is the same property the existing cursor tests check.
- **Agreement:** for tier 1 formulas, the Rust evaluator and the SQL translation
  give the same result on every fixture row. This keeps the two tiers in step.
- **Front end:** the Columns menu editor (validation, preview, presets) and
  layout migration from `aurora:songs-columns:v1`.
- **Performance:** a timing check on the release-size catalog for the
  acceptance check in §5.

---

## 9. Risks and open questions

- **Committing to a syntax.** Saved formulas make the syntax a public contract.
  Start small, with versioned definitions (`formula_version` in the table), and
  only add to it. Never change the meaning of an existing construct.
- **Unindexed full-catalog sorts.** Mitigated by the acceptance check and the
  fallback (sort only with a filter). If one formula becomes popular, it can be
  promoted to a built-in column with a proper index.
- **Rating overlays.** Pending Aurora ratings and Love edits are applied to the
  page *after* the SQL query (`apply_overlays` in `explorer.rs`). So SQL sorting
  uses the catalog value until Music Library has synced the edit. This is the
  same limitation the built-in Rating sort has today. Displayed computed values
  should be evaluated again in Rust after overlays, so the cell is always
  current even when its sort position lags. If that lag turns out to matter,
  overlays could be loaded into a temp table, the same way registered plays are
  (`temp.aurora_registered_track_plays`), and joined into the sort expression.
- **Open: should computed columns sync between devices?** They live in
  `StateStore`, so they could join state sync later. Laptop and Network Mode
  catalogs may lack some fields (for example sonic data), and those columns then
  show `—`.
- **Open: Albums first or Songs first?** This proposal starts with Songs,
  because its column system (0.50.0) already has show/hide, reorder and resize.
  Albums cards have no columns, so computed values there would need a different
  layout.

---

## 10. Prior art

- **JRiver Media Center:** expression language used for columns, grouping,
  titles, rename rules and smartlists
  ([wiki](https://wiki.jriver.com/index.php/Media_Center)).
- **Helium Music Manager:** user-configurable track columns and
  Content Filter grouping ([blog](https://implodedsoftware.wordpress.com/)).
- **MediaMonkey:** custom fields and auto-organize masks
  ([mediamonkey.com](https://www.mediamonkey.com/)).

Aurora would differ in two ways. Sortable formulas compile to bound-parameter
SQL within the existing keyset pagination. And formulas can never write
anything.
