# Logo construction

Rules for the marks of the apps that use this kit. ChronoRift's mark is the reference example
([`chronorift.svg`](chronorift.svg)). Every number here was measured on its source: the tile in
ChronoRift's `src-tauri/icons/icon.svg` and the `ICONS.logo` paths and comment in `src/dom.ts`.
Numbers are grid units unless they say px.

## Grid

- **24 × 24 grid with a margin of 2.** The mark lives in the 20 × 20 live area from 2 to 22.
- **Filled geometry only, with no strokes.** The shape then scales without stroke drift and exports
  cleanly to PNG, ICO and ICNS.
- **Coordinates are rounded to two decimals.**
- **Draw on [`template.svg`](template.svg).** It has the grid, the live area, the keylines, the 45°
  diagonals and the dimensions.

## Frame elements

| Rule | Value | ChronoRift, measured |
| --- | --- | --- |
| Thickness `t` | 2.6 (13 % of the live area) | Top and left bars from 2 to 4.6; bottom and right bars from 19.4 to 22. |
| Outer chamfer `c` | 45°, 18 % of the element side: 20 × 0.18 = 3.6 | `(2, 5.6)–(5.6, 2)` and `(22, 18.4)–(18.4, 22)`. |
| Inner chamfer `c′` | `c − (2 − √2)·t = c − 0.5858·t = 2.077`, drawn as 2.08 | `(4.6, 6.68)–(6.68, 4.6)` and `(19.4, 17.32)–(17.32, 19.4)`. The band stays `t` thick along the bevel. |
| 45° cut | Where a stroke ends against another element, its end is cut at 45°. The gap is `t` wide, measured perpendicular to the cut. | Cut lines `x + y = 22.16` and `x + y = 25.84`: `3.68 / √2 = 2.60`. |
| Where the cut sits | The gap is centered on a 45° diagonal through the grid center. | Centered on `x + y = 24`: `24 ± t/√2 = 24 ± 1.84`. |
| Along the edge | A gap `t` wide across a 45° cut is `t·√2 = 3.68` wide along the edge. | The top bar ends at `x = 20.16`, and the right bar starts at `y = 3.84`. |
| Gap between inner elements | `t/2` = 1.3 | Between the two arrows: `12.65 − 11.35 = 1.3`. |
| Clearance from inner elements to the frame | At least `t/2`. | 1.75 horizontally (`6.35 − 4.6`, `19.4 − 17.65`) and 3.3 vertically (`7.9 − 4.6`, `19.4 − 16.1`). |

## Color

- **Monochrome, always.**
  - On dark: ink `#EDEDEF` (the `--ink` token).
  - On light: `#0E0E10` (the `--bg-0` value).
  - Never the brand color, a gradient or a second color.
- **In the UI the mark is an inline SVG that inherits `currentColor`**, through `fill: currentColor` or `.icon--filled`.
  - ChronoRift: 20 px in the title bar (`.brand__mark`) and 44 px on the splash (`.splash__logo`).

## App tile

| Part | Value |
| --- | --- |
| Canvas | 1024 × 1024 |
| Tile | 944 × 944, inset 40 (a 3.9 % transparent margin), fill `#1E1E22` (the `--bg-3` value) |
| Tile chamfer | 170 = 18.0 % of 944: the same 18 % as the mark's corners |
| Mark box | The mark's 24-grid box is 640 wide (62.5 % of the canvas) at 192, so one grid unit is 26⅔ px. |
| Visible mark | 20 units = 533⅓ px, starting 205⅓ px inside the tile edge (21.75 % of the tile side) |

"About 19 % padding" is shorthand for `192 / 1024 = 18.75 %`, the offset of the mark's grid box from
the canvas edge. Measured from the tile edge, the grid box is inset `152 / 944 = 16.1 %`, and the
visible mark is inset `205⅓ / 944 = 21.75 %`. New tiles keep the same numbers: grid box 640 at 192
on a 1024 canvas.

- **Generate every platform icon from this one SVG.**
  - Run `bun run tauri icon <tile.svg>` in ChronoRift, or `npm run tauri icon <tile.svg>` in FairTranslator.
  - On Windows, tauri-build does not re-embed changed icons unless `tauri.conf.json` changes. Touch it before `tauri build`.

## Mandatory checks

- **Sizes.** Render the mark at 16, 20, 24 and 32 px, on dark and on light. [`chronorift.svg`](chronorift.svg) shows this ladder; view it at 100 %.
- **At 16 px one unit is 0.67 px.**
  - `t` is 1.73 px and `t/2` is 0.87 px, so gaps of `t/2` fall below one pixel and may close up.
  - In ChronoRift this is the gap between the arrows: 0.87 px at 16 px, 1.08 px at 20, 1.3 at 24 and 1.73 at 32.
  - The mark must still read when that gap blurs.
- **Silhouette.** Check it inside the app tile and in the 48 px title bar at 20 px.

## ChronoRift's mark

- **Two L-shaped frames**, top-left and bottom-right.
  - They are cut at 45° where they meet, on the diagonal from top-right to bottom-left.
  - They read as the League minimap with the client's chamfered corners. The shapes are original.
- **Two rewind arrows.**
  - Each is a triangle 5 wide and 8.2 tall, and they are `t/2` apart.
  - The pair is 11.3 wide, centered on (12, 12).
  - The arrows are specific to this mark; the rules above do not size them.

```text
M2 5.6L5.6 2H20.16L17.56 4.6H6.68L4.6 6.68V17.56L2 20.16Z        top-left frame
M22 18.4L18.4 22H3.84L6.44 19.4H17.32L19.4 17.32V6.44L22 3.84Z   bottom-right frame
M11.35 7.9V16.1L6.35 12Z                                          left arrow
M17.65 7.9V16.1L12.65 12Z                                         right arrow
```

## Adding a mark

1. Draw it on `template.svg`, in the `artwork` group, as filled paths.
2. Add `logo/<app>.svg` with the mark and its tile, laid out like `chronorift.svg`.
3. Run the checks above.
4. Commit with a CHANGELOG line.
