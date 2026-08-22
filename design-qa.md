# Design QA — Charts 0.15.0

## Evidence

- Reference: `C:\Users\jtill\.codex\generated_images\01a025c3-c6d3-73f1-ae98-1e18a3cabd80\exec-36252241-c641-45b3-a990-e559136426ba.png`
- Implementation: `C:\_code\aurora\docs\design\charts-0.15.0-implementation.png`
- Full comparison: `C:\_code\aurora\docs\design\charts-0.15.0-comparison.png`
- Focused chart comparison: `C:\_code\aurora\docs\design\charts-0.15.0-focused-comparison.png`
- Reference image: 1487 × 1058 physical pixels.
- Browser capture: 1280 × 720 CSS pixels at device-pixel ratio 1.25. The in-app Browser retained its 1280 × 720 viewport when a larger viewport was requested, so the reference was proportionally contained for the full comparison and the chart workspace was normalized separately for the focused comparison.
- State: Singles → Summer 1985 → Official UK → selected week 23 → Kayleigh selected.

## Comparison history

1. Initial implementation showed the selected design's main hierarchy but exposed period presets without an obvious complete-year action.
2. Added a compact, keyboard-accessible year control labelled “full year”; selecting it builds weeks 1–53 and switches to the calculated period chart.
3. Re-captured the exact weekly state and compared the reference and implementation side by side at full-shell and focused-workspace scales.

## Visible findings

- The final page preserves the reference hierarchy: Charts header, Singles/Albums switch, five period actions, calendar rail, chart-source rail, ranked table, right inspector, and fixed player.
- The selected row, purple emphasis, restrained cyan/green status accents, dark borders, compact type scale, and cover-driven table closely match Aurora's existing design language and the supplied mock.
- The implementation deliberately uses the album cover resolved from the user's real cover archive for Kayleigh rather than fabricating the standalone-single artwork shown in the concept.
- At 1280 × 720, the chart workspace scrolls vertically and six rows are visible above the fixed player. The full 1440 × 900 Tauri window exposes more of the source-comparison and Album Score sections without changing their hierarchy.
- No broken artwork, clipping across fixed shell boundaries, horizontal overflow, misplaced overlays, or low-contrast active states were observed.

## Required surfaces

- Loading: centered Aurora spinner and loading copy while a bounded chart request is pending.
- Empty: chart-specific empty explanation when a source/time lens has no entries.
- Error: retryable error panel using the existing Aurora feedback treatment.
- Main content: complete ranked chart with source, period, movement, match, playback, and inspector controls.
- Secondary/detail content: cross-source history, Aurora Album Score shelf, and selected track/album inspector.

## Primary interactions verified in the in-app Browser

- Navigate to Charts and select Kayleigh from the weekly Official UK chart.
- Switch Selected week to Period chart for Summer 1985.
- Click the displayed year to build the 1985 full-year chart.
- Switch Singles to Albums and load Aurora Album Score.
- Open Custom, enter August 1990 weeks 31–35, and apply the range.
- Confirm the page returned to the exact weekly comparison state after reload.
- Browser console was checked during the weekly, period, album, and custom-range flows; no errors or warnings were emitted.

## Result

passed
