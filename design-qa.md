# Aurora 0.13.0 Years design QA

## Inputs

- Release-lens reference: `docs/design/years-0.13.0-release-lens.png`.
- Original-lens reference: `docs/design/years-0.13.0-original-lens.png`.
- Implementation: live Aurora Browser preview at the matching 1536 × 1024 desktop viewport, with the left Library rail, Album inspector, and persistent player visible.
- States: Release Year 2025 and Original Year 1982, both in Two Clocks mode with Blade Runner selected.
- Comparison method: each full-resolution reference and its matching live Browser screenshot were inspected together in the same comparison input.

## Comparison history

1. The initial paired implementation established the reference's midnight shell, cyan Original Year clock, violet Release Year clock, flowing edition relationships, grouped cover shelf, inspector, and persistent player.
2. The first live comparison found a P2 label collision near the selected Release Year marker. The duplicate floating label was removed, its accessible live announcement was retained, and the selected-year labels were repositioned beside their clock markers.
3. The final comparisons preserve the two distinct date authorities, selected-year glow, directional aggregated flows, compact edition groupings, purple primary actions, and right-side Album context. Packaged Aurora replaces the Browser preview's labeled generated covers with the user's real cover archive through the existing album-art protocol.

## Responsive and interaction checks

- At the normal 1280 × 720 Browser viewport, `body.clientWidth` and `body.scrollWidth` both measured 1280 px; no page-level horizontal overflow was observed.
- Release landscape, Original landscape, and Two Clocks all switch to their correct semantic chart.
- Selecting Release Year 2025 and Original Year 1982 redraws the flows, shelf, summary, and selected Album context.
- Missing Original Year opens its separate lens; Explore hands Songs `yearBasis = original` with the missing filter enabled.
- Explore Original 1982 hands Songs exact `1982` lower and upper bounds with `yearBasis = original`.
- Play Release 2025 populated a bounded 64-track preview queue and started the matching representative track.
- Browser console check after the complete interaction sequence: no warnings or errors.
- The live read-only 72,012-album catalog overview test completed its paired query in approximately 0.34 seconds and returned bounded timelines and at most 100 albums.

## Final findings

- P0: none.
- P1: none.
- P2: none.
- P3: Browser preview uses labeled generated album covers because the contained native artwork protocol is available only in packaged Tauri; this does not affect the packaged release.

Final result: passed
