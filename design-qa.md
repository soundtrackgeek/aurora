# Design QA — Publishers 0.15.16

## Evidence

- Reference: `C:\Users\jtill\.codex\generated_images\01a02f7b-8e99-7623-9d3c-ed7dc563d8ea\exec-6dad8c97-922a-4848-92e2-8ecef4279fef.png`
- Implementation: `C:\_code\aurora\docs\design\publishers-0.15.16-implementation.png`
- Side-by-side comparison: `C:\_code\aurora\docs\design\publishers-0.15.16-comparison.png`
- Both captures: 1487 × 1058 CSS pixels.
- State: Publishers → Parlophone → Release activity → selected Plastic Beach album.

## Comparison

- The implementation preserves the reference hierarchy: Library destination, publisher search, three timeline lenses, six publisher signal rows, selected-publisher summary, decade highlights, album inspector, and fixed player.
- The row spacing, compact typography, purple selected signal, cyan secondary signals, quiet grid, dark borders, and inspector metadata follow the supplied design and Aurora's existing palette.
- Real catalog mode uses bounded album-level publisher rollups and real album covers. Browser preview mode deliberately uses Aurora's existing cover fixtures and the same component boundaries.
- Publisher logos are not fabricated or bundled. The circular logo slots use an intentional Aurora fallback while the documented MusicBrainz → Wikidata → Wikimedia Commons enrichment path remains optional future work.
- At the exact reference viewport, the page stays inside the shell with no horizontal overflow, overlapping panels, cropped publisher signals, or player/inspector boundary errors.

## Interactions verified in the in-app Browser

- Navigate to Publishers from the expanded Library group.
- Switch Release activity to Original-year activity.
- Filter the publisher list to Warp and clear the filter.
- Select Warp Records and a decade-highlight album.
- Confirm the album inspector shows `Publisher: Warp Records`.
- Use Explore publisher and confirm the Songs handoff applies `publisher:"Warp Records"`.
- Check browser logs after the complete flow; no errors or warnings were emitted.

## Required states

- Loading: bounded publisher-rollup and detail feedback.
- Empty/error: publisher-specific retryable feedback without falling into the ordinary Explorer.
- Main: six publisher activity rows with search and three functional timeline modes.
- Detail: selected publisher metrics, bounded decade shelf, publisher playback, and publisher-scoped Explore handoff.
- Inspector: selected album artwork and Publisher metadata.

## Result

passed
