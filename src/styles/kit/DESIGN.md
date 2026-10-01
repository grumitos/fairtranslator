# Design kit 0.1.3

This kit holds the shared dark design of two Tauri desktop apps.
- **ChronoRift** (vanilla TypeScript, plain CSS) is the source: tokens and components were extracted from its `src/styles.css`.
- **FairTranslator** (React 19, Tailwind 4, shadcn/ui) maps the same tokens in `tailwind.css`.

| File | What it holds |
| --- | --- |
| `tokens.css` | Colors, lines, inks, semantic states, brand slot, radii, spacing, fonts, motion, focus. Dark only. |
| `base.css` | Body typography, focus ring, selection, scrollbars, reduced-motion policy. |
| `components.css` | Vanilla components with ChronoRift's class names. |
| `tailwind.css` | Tailwind 4 and shadcn bridge, for FairTranslator. |
| `logo/` | Logo construction rules, drawing template, ChronoRift's mark as the example. |
| `check.mjs` | The brand and contrast check; the vendored copy runs in each app's CI. |

How to vendor and check the kit: see `README.md` in the kit repo. All measurements are in px.

## 1. Rules

1. **Monochrome, utilitarian base.**
   - Surfaces, lines and text are neutral grays.
   - Color appears only for a **state** (success, danger, warning, info) or for the app's **identity** (the brand slot, within its closed list).
   - Nothing is colored for decoration.
2. **No box inside a box.**
   - Data fields (a label and its value) sit on the page, separated by space and 1px lines, never inside a card or a capsule.
   - Only buttons are boxes.
   - A panel (drawer, card) is the one surface; what is inside it doesn't get another border.
   - Form controls (inputs, selects) and chips keep their own outline, because it is part of the control.
3. **Copy is labels and data only.**
   - No explanatory sentences, no "haz clic aquí", no tips.
   - Neutral Spanish that reads the same everywhere, with **tú** and no voseo.
   - Where the user must choose, a short question and two labels are enough.
4. **Motion is brief and nothing jumps.**
   - Color changes fade in 120 ms. Large layers enter in 260 ms and leave in 180 ms.
   - Text that arrives late has its space reserved: fixed-height lines, minimum widths, tabular figures.
   - Images fade in their already-sized slot instead of popping.
5. **Visible focus and full keyboard access.**
   - Every interactive element shows the 2px ink ring on `:focus-visible` and can be reached and used with the keyboard.
6. **Utilitarian, pleasant, compatible colors.**
   - The palette is calm and tested for contrast.
   - The brand accent is used sparingly and never competes with states.

### Copy examples

| Case | Write | Avoid |
| --- | --- | --- |
| Button | `Descargar` · `Abrir carpeta` · `Copiar ruta` | `Haz clic para descargar el cliente` |
| State | `Listo` · `Incompleto` · `Descargando · 42 %` | `La descarga se está realizando, espera…` |
| Empty | `Sin carpeta` · `Sin resultados` | `Todavía no elegiste ninguna carpeta` (voseo, and a sentence) |
| Question | `¿Descargar todas las imágenes ahora?` | `¿Querés descargar todo ahora?` (voseo) |
| Error | `Ya hay una descarga en curso.` · `La carpeta no está disponible: E:\Juegos` | `Ups, algo salió mal` |
| Addressing the user | `No modifica tu instalación.` | `No modifica su instalación.` (usted) |
| Missing value | `—` | `N/A`, `desconocido` |

Numbers follow the interface locale (`es` or the system's Spanish variant), with a no-break space
before units and percent: `1 163`, `12 MiB`, `42 %`, `8,2 k`. Sizes use binary units (KiB, MiB, GiB).
A progress value never shows `100 %` before the work is done.

## 2. Color

### Base

| Group | Tokens | Use |
| --- | --- | --- |
| Surfaces | `--bg-0`, `--bg-1`, `--bg-2`, `--bg-3`, `--bg-4` | Page; panels; hover and field; selected; active segment |
| Field surface | `--bg-field` | Inputs, selects, search, textareas (= `--bg-2`) |
| Overlay | `--scrim`, `--shadow-drawer`, `--selection` | Behind a drawer; drawer shadow color; text selection |
| Tiles | `--tile`, `--tile-line` | Image slots before the image paints |
| Lines | `--line`, `--line-soft`, `--line-field`, `--line-strong`, `--line-chip`, `--line-dim`, `--line-track` | Divider; row separator; field; button; outline chip; disabled; pending track |
| Inks | `--ink`, `--ink-hover`, `--ink-2`, `--ink-3`, `--ink-4` | Primary; ink-button hover; secondary; labels; disabled only |

- Every token carries its role in a comment in `tokens.css`.
- The text contrast on `--bg-0` to `--bg-3`:
  - `--ink`: at least 14.2:1.
  - `--ink-2`: at least 7.4:1.
  - `--ink-3`: at least 4.9:1. It drops to 4.2:1 on `--bg-4`, so an active segment uses `--ink-2` for its count.
  - `--ink-4`: disabled text only.

### Semantic states

| Token | Value | Meaning | ChronoRift uses it for |
| --- | --- | --- | --- |
| `--success` | `#3DD68C` | Done, ready, positive | Victory, ready client |
| `--danger` | `#FF8F8F` | Error, failure, negative | Defeat, error lines |
| `--warning` | `#FFC53D` | Incomplete, blocked, unavailable | Incomplete client, Vanguard, missing drive |
| `--info` | `#70B8FF` | In progress, neutral notice | Download in progress |

- **Variants.**
  - Each state has `-bg`, a tint behind a chip, and `-line`, the chip's outline.
  - `--danger-bg` and `--danger-line` derive from the deeper red `#E5484D`, as in ChronoRift.
  - `--info-bg` and `--info-line` are new in the kit; ChronoRift's info chip is text-only.
- **Contrast.** Every state text passes 4.5:1 on `--bg-0` to `--bg-3`; the lowest is danger on `--bg-3`, at 7.6:1.
- **Only for states, and never the only signal.** A colored chip, glyph or text always sits next to its word, letter or shape (`V`/`D`, `Listo`, the glyph's shape).
- **App-specific colors stay in each app.** For example, ChronoRift's League team colors.

### Brand slot

| Token | Role |
| --- | --- |
| `--brand` | Fill of the primary action, progress fills, the active indicator, the done glyph |
| `--brand-ink` | Text and icons on a `--brand` fill |
| `--brand-text` | Brand-colored text on `--bg-0` to `--bg-3`; the text of verbatim tokens (e) |
| `--brand-soft` | Brand tint; the background of verbatim tokens (e) |
| `--brand-line` | Brand outline (unused by kit components so far) |

- **The kit's own values are a neutral ink fallback.**
  - Unbranded, the primary action is an ink-filled button and progress fills are ink.
  - Each app sets the five tokens in its own CSS, after importing `tokens.css`.
- **Closed list of uses. The brand appears only in:**
  - (a) the single primary action of each view, as a filled button (`.button--brand`);
  - (b) progress fills (`.meter__fill`);
  - (c) the 2px active tab and nav indicator (`.tab[aria-pressed="true"]`, `.nav-item[aria-current="page"]`). Segmented controls stay neutral;
  - (d) the "done" step glyph (`.stages li[data-status="done"]`);
  - (e) inline tokens the user must preserve verbatim, such as placeholders and control codes (FairTranslator shows RPG Maker codes like `\C[2]`, `\N[1]`, `\V[3]` this way): `--brand-text` on `--brand-soft`, in mono at 12px. Never as buttons or states. They are data that color makes easier to spot, not decoration.
- **Never brand:**
  - logos, which are always monochrome ink;
  - the focus ring, which is ink;
  - states, links, hover, selection, checkboxes, switches and selected rows.

**Examples.** These are app values, not the kit default.

ChronoRift, muted Hextech gold:

```css
:root {
  --brand: #c8aa6e;                      /* oklch(0.751 0.086 83.9) */
  --brand-ink: #1a1408;                  /* 8.2:1 on the gold */
  --brand-text: #c8aa6e;                 /* 8.7:1 on --bg-0 */
  --brand-soft: rgba(200, 170, 110, 0.14);
  --brand-line: rgba(200, 170, 110, 0.4);
}
```

FairTranslator, indigo (`src/styles/brand.css`):

```css
:root {
  --brand: #999fdc;                      /* oklch(0.721 0.089 279.6) */
  --brand-ink: #111221;                  /* 7.4:1 on the indigo */
  --brand-text: #999fdc;                 /* 7.7:1 on --bg-0 */
  --brand-soft: rgba(153, 159, 220, 0.14);
  --brand-line: rgba(153, 159, 220, 0.4);
}
```

### Family rule

The apps' brand accents belong to one family: they share OKLCH lightness **L 0.70–0.76** and
chroma **C 0.07–0.10**, and only the hue changes.

### Separation from the states

The brand must never be mistaken for a state color. The check measures **ΔE_OK**, the perceptual
distance between `--brand` and each of success, danger, warning and info. ΔE_OK is the Euclidean
distance in OKLab.

| ΔE_OK | Result | Meaning |
| --- | --- | --- |
| Below 0.075 | Fail | Confusable: change the brand. |
| 0.075 up to, but not including, 0.10 | Warning | Related colors. The check passes, but the brand and that state must be used in roles that never cross. |
| 0.10 or more | Ok | Distinct. |

Hue alone misjudges this. Lightness and chroma separate colors too:

- **Same hue, distinct colors.** ChronoRift's gold and warning share a hue (83.9° and 84.1°), yet they are as far apart as gold and danger: ΔE_OK 0.125 in both cases. Warning is lighter (ΔL 0.103) and more chromatic (ΔC 0.072).
- **30° apart in hue, yet the closest pair.** FairTranslator's indigo and info differ by 30.1° in hue, yet they are the closest brand-state pair of both apps, at ΔE_OK 0.079. Their lightness and chroma are close (ΔL 0.044, ΔC 0.037).

| Brand | Success | Danger | Warning | Info |
| --- | --- | --- | --- | --- |
| ChronoRift, gold `#C8AA6E` | 0.166 | 0.125 | 0.125 | 0.210 |
| FairTranslator, indigo `#999FDC` | 0.233 | 0.183 | 0.278 | 0.079 (warning) |

FairTranslator accepts its warning. There, info appears only in status toasts, and never in the
brand uses: the primary action, progress fills, the active indicator, the done glyph and the verbatim tokens.

### The check

`node scripts/check.mjs --brand-from <app.css>`, or the vendored `check.mjs` (see the README), checks:
- the brand's L and C band;
- ΔE_OK between the brand and each state, printed with ΔL, ΔC and Δhue for context;
- 4.5:1 for `--brand-text` and each state text on `--bg-0` to `--bg-3`;
- 4.5:1 for `--brand-ink` on `--brand`.

It prints a table and counts warnings apart from failures. It exits non-zero only on a failure.
At 0.1.2, ChronoRift passes every check, and FairTranslator passes with the one warning above.

## 3. Typography

- **Fonts.**
  - UI text: Geist (`--font-sans`).
  - Data: Geist Mono (`--font-mono`), with the `.mono` class and tabular figures: paths, builds, sizes, counts, ids, dates, percentages.
- **Body.**
  - 13px, `font-synthesis: none`, default line height.
  - The 13px sits on `body`, so `rem` stays at 16px. Kit sizes are in px, so a rem-based utility is not a kit size.
- **Weights.** 400 body; 500 controls (buttons, tabs, segments, nav); 600 titles and key figures; 700 chips.

| Size | Weight | Use |
| --- | --- | --- |
| 10.5 (mono, uppercase, +0.08em) | 700 | Chip (10 in `.chip--sm`) |
| 11.5 | 400 | Labels (`dt`), captions, table heads, counts |
| 12 | 400 | Secondary text, small buttons, subtitles |
| 12.5 | 400–500 | Dense data, selects, segments, key-value values |
| 13 | 400–500 | Body, buttons, tabs |
| 13.5 | 400 | Inputs |
| 14 (−0.01em) | 600 | App name in the title bar |
| 15 (−0.01em) | 600 | Panel and drawer titles |
| 22 (−0.015em) | 600 | Page title, key figure in a drawer |
| 28–32 (−0.02em) | 600 | Hero figure (progress percent) |

## 4. Spacing and radii

- **Spacing: `--space-2`, `-4`, `-6`, `-8`, `-10`, `-12`, `-16`, `-20`, `-24`, `-32`, `-40`.**
  - Each is named by its px value.
  - Layout between components uses these steps.
  - Component internals keep the exact values in the table below, including a few optical ones: 3, 7, 9 and 14.
- **Radii.**
  - `--radius-sm` 4: chips, kbd.
  - `--radius-md` 6: compact controls of 24–32px.
  - `--radius` 7: 34px controls and tiles.
  - `--radius-lg` 10: cards, menus, toasts.
  - `--radius-pill`: meters, switches.
- **Nested radii.** Smaller controls take smaller radii, and an outer radius is the inner one plus the padding between them. The segmented track is 6 + 2 = 8.

## 5. Components in `components.css`

Heights, paddings and font sizes are px. The focus ring is the global one unless the row says otherwise:
2px `--ink`, offset 2px. Transitions list duration, then curve.

| Component | Height | Padding (gap) | Radius | Font | Focus ring | Transition | Reduced motion |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `.button` | 34 | 0 12 (7) | 7 | 13 / 500 | global | bg, border, color 120 ease | unchanged: color only |
| `.button--sm` | 28 | 0 9 | 7 | 12 / 500 | global | same | unchanged |
| `.button--square` | 34 × 34 | 0 | 7 | icon 16, `--ink-2` | global | same | unchanged |
| `.button--primary` (ink fill) | 34 | 0 12 | 7 | 13 / 500 | global | same; hover `--ink-hover` | unchanged |
| `.button--brand` (primary action) | 34 | 0 12 | 7 | 13 / 500 | global | same; hover `brightness(1.08)`, not animated | unchanged |
| `.icon-button` | 30 × 30 | 0 | 6 | icon 16 | global | 120 ease | unchanged |
| `.icon-button--sm` | 26 × 26 | 0 | 6 | icon 15–16, `--ink-3` | global | 120 ease | unchanged |
| `.icon-button--xs` | 24 × 24, margin −3 0 | 0 | 6 | icon 15, `--ink-3` | global | 120 ease | unchanged |
| `.chip` | 20 | 0 7 | 4 | mono 10.5 / 700 | not focusable | none | — |
| `.chip--sm` | 18 | 0 6 | 4 | mono 10 / 700 | — | none | — |
| `.segmented` (track) | 34 | 2 (2), 1px `--line` | 8 | — | — | — | — |
| `.segmented button` | 28 | 0 6 (6) | 6 | 12.5 / 500; count 11.5 | global | 120 ease | unchanged |
| `.select` | 32 | 0 34 0 10; chevron 14 at 10 | 6 | 12.5 | global | none | — |
| `.input` | 34 | 0 12 | 7 | 13.5 | global | none | — |
| `.search input` | 35 | 0 10 0 33; icon 15 at 10, 10 | 7 | 13 | global | none | — |
| `.tabs` / `.tab` | 40 | 0 12 (4) | 0 | 13 / 500 | global | 120 ease | unchanged |
| `.appbar` | 48 | 0 0 0 16 (24) | 0 | name 14 / 600 | — | — | — |
| `.nav-item` | 30 | 0 12 (2) | 6 | 13 / 500 | global | 120 ease | unchanged |
| `.window-control` | 48 × 46 | 0 | 0 | icon 14, stroke 1.4 | inset: offset −2 | 120 ease | unchanged |
| `.drawer` | full height × `min(440px, 100%)` | head 16 20 14 (12); body 20 (16); foot 16 20 (8) | 0 | title 15 / 600; subtitle 12 | panel `outline: none`; focus goes to its close button | in: transform 260 `--motion-ease-enter`; out: 180 `--motion-ease-exit` | no slide: opacity 120 ease |
| `.drawer-scrim` | covers the container | — | — | — | — | in: opacity 260 ease-out; out: 180 ease-in | unchanged: a fade |
| `.meter` | 6 | — | pill | — | — | none: the width follows real progress | — |
| `.meter--mini` | 4 × 64 | — | pill | — | — | none | — |
| `.tile` | 36 × 36 | — | 7 | initials 12 / 600 | — | image opacity 120 ease | unchanged: a fade |
| `.glyph` | 8 × 8 | — | 50 % | — | — | busy: rotation 1 s linear, looping | busy is static |
| `.stages li` | 36 | (12); glyph 16 | glyph 50 % | 13 | — | none | — |
| `.facts` | `dd` 18, line 18 | (12 32); `dt` to `dd` 3 | — | `dt` 11.5 `--ink-3` | — | — | — |
| `.kv__row` | auto | 10 0 (16); stack 12 0 (8); compact 8 0 | — | `dt` 12 `--ink-3`; `dd` 12.5 | — | — | — |
| `.empty` | fills its parent | 24 (12) | — | 13 `--ink-3`; detail 12 | — | — | — |
| `.splash` | full window | logo 44, top edge 68 above center; body from center + 4 | — | — | window controls stay above it | out: opacity 180 ease | unchanged: a fade |
| `.dots` | 6 × 6 dots | 8 0 (6) | 50 % | — | — | opacity 0.25 → 1, 1.2 s ease-in-out, staggered 0.16 s | static at opacity 0.6 |
| `.skip-link` | auto | 7 10 | 7 | 13 | global | none | — |

### States

- **Button**
  - Hover: fill `--bg-2`.
  - Disabled: border `--line-dim`, fill `--bg-2`, text `--ink-4`, cursor `not-allowed`. The brand and ink fills look like any disabled button.
  - `.button__icon` is a slot for an icon that changes with the action.
- **Icon button**
  - At rest `--ink-2`. The `--sm` and `--xs` sizes rest at `--ink-3`.
  - Hover: fill `--bg-3`, text `--ink`. Disabled: `--ink-4`.
  - Always add `aria-label` and `title`, in Spanish: `Cerrar`, `Abrir carpeta`.
- **Chip**
  - Neutral: `.chip--solid` (ink fill) and `.chip--outline` (`--line-chip` border, `--ink-2` text).
  - Tones, set by class from the state and never from the text:
    - `is-success`, alias `is-win`;
    - `is-danger`, alias `is-loss`;
    - `is-warning`, alias `is-warn`;
    - `is-info`, text color only.
- **Segmented**
  - A group with `role="group"` and a label. Each segment is a `<button>` with `aria-pressed`.
  - The active segment is `--bg-4` with `--ink` text. It is never brand.
- **Select field**
  - Markup: `<div class="select-field"><select class="select">…</select><svg class="icon select-field__icon">` holding the chevron path `M7 10l5 5 5-5`.
  - Disabled: text `--ink-3`, chevron `--ink-4`.
- **Tabs**
  - A `<button class="tab">` with `aria-pressed="true"` (ChronoRift), or `role="tab"` with `aria-selected="true"`.
  - The active tab gets `--ink` text and an inset 2px `--brand` line at its bottom.
- **Nav item**
  - `aria-current="page"` gets `--ink` and a 2px brand line on the bar's bottom edge.
  - That line sits 9px below the item because the item is 30 in a 48 bar. Change both together.
- **Drawer**
  - Markup: `.drawer-scrim` and `.drawer` are siblings inside a `position: relative; overflow: clip` container, with `role="dialog"` and `aria-labelledby` on `.drawer`.
  - To open: remove `hidden`, force a style read, then add `is-open` to both. Focus the close button.
  - To close: remove `is-open`, make them `inert`, and set `hidden` when the transitions finish. Return focus to the trigger.
  - The content behind is `inert`, and Escape closes.
- **Meter**
  - The track is `--line` (`--bg-4` for `--mini`); the fill is `--brand`.
  - The script sets the fill's width from real progress, never from a timer.
  - A meter that appears must not move anything; ChronoRift places it `position: absolute` under the status text.
- **Glyph** (`data-state`)
  - `ready`: filled `--success`.
  - `partial`: `--warning` ring, half filled.
  - `busy`: `--info` ring with a `--line-track` quarter, spinning.
  - `missing`: `--ink-3` ring.
  - `blocked`: `--warning` ring with a diagonal bar.
  - `unknown`: a faint `--line-strong` dot, meaning not read yet.
- **Step list** (`data-status`)
  - Pending: `--line-track` ring, `--ink-3` text.
  - `active`: 2px ink ring with a 3px ink dot, `--ink` text.
  - `done`: a `--brand` disc with a `--brand-ink` check drawn as a mask, `--ink` text.
- **Image fade-in**
  - Give the image slot its final size.
  - If `img.complete` is false, add `is-pending` and remove it on `load`. On `error`, replace the image with the slot's initials.
- **Splash**
  - `.splash` is `position: fixed`, with z-index 30 and `data-tauri-drag-region="deep"`.
  - `.window-controls` (z-index 31) stay usable above it.
  - Add `is-leaving` to fade it out, then set `hidden`.

## 6. Specs without CSS yet

These are the measurements and states of components that FairTranslator needs and ChronoRift lacks.
They use the tokens above; CSS arrives in a later kit version. None of them uses the brand.

### Dense data table

| Part | Spec |
| --- | --- |
| Header row | 28px, sticky, on the table's surface; text 11.5 / 400 `--ink-3`; bottom border 1px `--line` |
| Sorted column | Header text `--ink-2`, chevron 12 `--ink-2`, `aria-sort` on the header cell |
| Body row | 28px fixed height, one line, so virtualized lists can size rows; text 12.5 `--ink`; secondary columns `--ink-2` |
| Cells | Padding 0 8; numbers, ids and paths in `.mono`; numbers right-aligned; long values end in an ellipsis and show in full in a `title` or a tooltip |
| Separators | 1px `--line-soft` between rows; no vertical rules |
| Hover | Row fill `--bg-2`, square corners |
| Selected row | Fill `--bg-3` with `aria-selected="true"`, also under hover; multi-select adds a 16px checkbox column as the non-color signal |
| Focused cell | Ring 2px `--ink`, offset −2 (inset), on the cell. The grid is one tab stop: arrows, Home and End, Page Up and Page Down move; Enter opens or edits |
| Empty | `.empty` in the body: `Sin resultados` |
| Loading | Keep the header and the row height; no spinner per row; the three-dot loader centered in the body |

### Textarea

| Part | Normal | Mono |
| --- | --- | --- |
| Font | 13.5 `--font-sans` | 12.5 `--font-mono`, tabular figures |
| Line height | 20px | 20px |
| Box | Padding 7 12, radius 7, 1px `--line-field`, `--bg-field`, `resize: vertical` | same |
| Height | One row = 34px, the input height; minimum 3 rows = 74px | same |

- **Placeholder:** `--ink-3`.
- **Focus:** the global ring on the box.
- **Disabled:** `--ink-3` text, `not-allowed`.
- **Read-only:** same box, `--ink-2` text.
- **Invalid:** 1px `--danger-line` border, and a message in `--danger` at 11.5 on an 18px line below. Reserve that line so the message doesn't move the form.
- **No hover change.**

### Checkbox, switch and radio in rows

- **Row.**
  - 32px minimum, padding 0 8, gap 10, radius 6, hover fill `--bg-2`.
  - The whole row is the `<label>`. Label 13 `--ink`; optional value 12 `--ink-3`, right-aligned.
  - Disabled row: `--ink-4`, no hover.
  - Rows sit on the page with no box around the group; a group label is 11.5 `--ink-3`.
- **Checkbox.**
  - 16 × 16, radius 4, border 1.5px `--ink-3`.
  - Checked: fill and border `--ink`, with a 12px check in `--bg-0`, stroke 2.5. Indeterminate: the same fill with an 8 × 2 dash.
  - Disabled: border `--line-dim`, fill `--bg-2`, mark `--ink-4`.
  - Focus: ring on the box.
- **Radio.**
  - 16 × 16 circle, border 1.5px `--ink-3`.
  - Selected: a 2px `--ink` ring and a 6px `--ink` dot, like the active step.
  - Arrows move within the group; Space selects.
- **Switch.**
  - Track 28 × 16, pill.
  - Off: track `--bg-4`, 12px thumb `--ink-3` at 2px from the left.
  - On: track `--ink`, thumb `--bg-0` moved 12px right.
  - Thumb and colors animate over 120 ease. With reduced motion, the thumb jumps and the colors still fade.
  - `role="switch"` with `aria-checked`. The label names what it controls, not `Sí`/`No`.

### Tooltip

- **Surface.**
  - `--bg-3`, 1px `--line-strong`, radius 6, padding 4 8, text 12 `--ink`.
  - Maximum width 320, no arrow, no shadow, 6px from its anchor.
- **Timing.** It opens after 600 ms of hover, or at once on keyboard focus. It closes at once on leave, blur or Escape. Moving to a neighbor within 300 ms skips the delay.
- **Motion.** Opacity over 120 ease, no movement, also with reduced motion.
- **Content.** A label or data only: a full path, a shortcut in `kbd`.
- **Limits.** Never interactive, and never the only place where information lives.

### Menu and popover

- **Surface.**
  - `--bg-2`, 1px `--line-field`, radius 10 (`--radius-lg`).
  - Shadow `0 12px 32px var(--shadow-drawer)`.
  - Padding 4 for a menu, 12 16 for a popover.
- **Menu item.**
  - 30px, padding 0 10, gap 8, radius 6 (10 − 4, nested).
  - Text 13 `--ink-2`, icon 16 `--ink-3`.
  - Highlighted by pointer or keyboard: fill `--bg-3`, text and icon `--ink`.
  - Shortcut: `.mono` 11.5 `--ink-3`, right-aligned.
  - Disabled: `--ink-4`.
  - Destructive: text and icon `--danger`, with a label that says it (`Eliminar`).
  - Checkable items reserve a 16px slot on the left, so checking one shifts nothing.
- **Separator and group label.** A separator is 1px `--line` with 4px above and below. A group label is 24px, 11.5 `--ink-3`, padding 0 10.
- **Size.** Width 180–320. The height is limited to the space available, and the menu scrolls inside.
- **Keyboard.** Arrows, Home and End, and typeahead move. Enter and Space choose. Escape closes and returns focus to the trigger; Tab closes.
- **Motion.** Opacity over 120 ease in and out, no scale, also with reduced motion.
- **Popover.**
  - 280–360 wide, holding facts, key-value rows or fields.
  - Focus moves inside. Escape or a click outside closes it and returns focus.

### Toast tokens

These are proposed names and values; they are not in `tokens.css` yet.

| Proposed token | Value | Role |
| --- | --- | --- |
| `--toast-width` | 360px | Width |
| `--toast-inset` | 16px | Distance from the window's bottom and right edges |
| `--toast-gap` | 8px | Between stacked toasts; at most 3 visible |
| `--toast-bg` | `var(--bg-2)` | Surface |
| `--toast-line` | `var(--line-strong)` | 1px border |
| `--toast-radius` | `var(--radius-lg)` | 10px |
| `--toast-shadow` | `0 12px 32px var(--shadow-drawer)` | Separation from the page |
| `--toast-enter` | `var(--motion-slow) var(--motion-ease-enter)` | Opacity and translateY(8px) to 0 |
| `--toast-exit` | `var(--motion-base) var(--motion-ease-exit)` | Opacity |
| `--toast-timeout` | 5000ms | Success and info. Warning waits 8000ms; danger stays until dismissed. Pause on hover and focus |

- **Layout.**
  - Padding 12 14, gap 10.
  - A 16px glyph or icon in the state color.
  - A title at 13 / 500 `--ink`, and one data line at 12 `--ink-2` with an ellipsis.
  - An optional `.button--sm` and a close `.icon-button--sm`.
- **Reduced motion.** Opacity over 120, with no translation.
- **Accessibility.** The region is `aria-live="polite"`; a danger toast uses `role="alert"`.
- **Copy.** The title is the state (`Descarga detenida`) and the line is the data (`16.18.817.5716 · 1,2 GiB`).

### kbd chip

- **Box.** `inline-flex`, height 18, minimum width 18, padding 0 5, radius 4, 1px `--line-strong`, `--bg-2`.
- **Text.** `.mono` 11 / 500 `--ink-2`, centered.
- **Combinations.** Keys sit side by side with a 4px gap and no `+`: `Ctrl` `K`.
- **Key names.** Use the names printed on the keys: `Ctrl`, `Alt`, `Mayús`, `Supr`, `Esc`, `Tab`, `F2`.

### Inline code

- **Text.** `.mono` 12px (0.92em at 13), `--ink`.
- **Box.** Padding 1 4, radius 4, fill `--bg-3`, no border.
- **Long values.** Paths and keys wrap with `overflow-wrap: anywhere`.

### Resize handle

- **Hit area.** 8px across a 1px `--line` divider, centered on it: 8 wide for columns and panels, 8 tall for rows. Cursor `col-resize` or `row-resize`.
- **Feedback.**
  - Hover, after 150 ms, and while dragging: the line turns 2px `--ink-3`.
  - Keyboard focus: a 2px `--ink` line.
  - Color changes over 120 ease. The handle follows the pointer with no animation.
- **Keyboard.**
  - `role="separator"`, `aria-orientation`, `aria-valuenow`, `aria-valuemin`, `aria-valuemax` and `tabindex="0"`.
  - Arrows move 8px, and with Mayús 32px. Home and End go to the limits. Enter or a double-click restores the default size.
- **Limits.** Panels keep their minimum sizes, and each app remembers the chosen size.

## 7. Motion

| Token | Value | Use |
| --- | --- | --- |
| `--motion-fast` | 120ms | Hover and press color changes, image fade-in, reduced-motion fades, menus, tooltips |
| `--motion-base` | 180ms | Exits of large layers (drawer, scrim, splash); delay before a busy state |
| `--motion-slow` | 260ms | Entrances of large layers (drawer, scrim, toast) |
| `--motion-ease` | `ease` | Color and opacity |
| `--motion-ease-enter` | `cubic-bezier(0.16, 1, 0.3, 1)` | Decelerates into place |
| `--motion-ease-exit` | `cubic-bezier(0.4, 0, 1, 1)` | Accelerates away |

- **Timing.**
  - Exits are shorter than entrances.
  - Nothing animates layout: width, height or position of content.
  - Progress fills follow real progress with no easing or fake timers.
- **A state that may resolve at once shows its busy look only after a delay.**
  - ChronoRift fades a view to 0.6 opacity, and only after 180ms, so a fast result never flickers.
- **Late text never moves the layout.** Reserve its place:
  - 18px data lines;
  - `min-width` for labels that change (`Descargando · 100 %`, `5ch` for `100 %`);
  - tabular figures;
  - a reserved line for the file in progress.
- **Reduced motion.** `base.css` makes keyframe animations end at once and turns off smooth scrolling. Components add:
  - a drawer that fades (120) instead of sliding;
  - a busy glyph that stops;
  - dots that stay static at opacity 0.6.

  Color and opacity transitions stay. Any new component that slides, scales or loops must define its own reduced-motion state.

## 8. Focus and keyboard

- **The ring.**
  - `:focus-visible` draws `--focus-ring-width` (2px) of `--focus-ring-color` (ink), offset 2px.
  - Controls flush with an edge, or inside a clipped list, draw it inside with an offset of −2px (window controls, list rows).
- **Programmatic focus targets.**
  - Containers that only receive focus from code (a drawer, the main region behind the skip link) may use `outline: none`.
  - Nothing else removes the ring.
- **Skip link.** The first element in the body is `<a class="skip-link" href="#main">Ir al contenido</a>`.
- **Dialogs and drawers.** Focus moves in, the page behind is `inert`, Escape closes, and focus returns to the trigger or to the nearest sensible control.
- **Lists that re-render keep focus on the same item.** Reconcile rows by a stable key.
- **Semantics.**
  - Segmented: `role="group"` and `aria-pressed`.
  - Tabs: `aria-pressed`, or `role="tab"` and `aria-selected`.
  - Progress: `role="progressbar"` with values, and `role="status"` for the percent.
  - Errors: `role="alert"`.

## 9. Window (Tauri 2)

The window entry in `tauri.conf.json` (`app.windows[0]`):

```json
{
  "decorations": false,
  "shadow": true,
  "backgroundColor": "#0E0E10",
  "minWidth": 1116,
  "minHeight": 689,
  "center": true
}
```

- **No white flash.**
  - `backgroundColor` is `--bg-0`. The native window and WebView2 paint it before the page loads.
  - `index.html` carries `<meta name="color-scheme" content="dark">`, `<meta name="theme-color" content="#0E0E10">`, and an inline `<style>` with `html { background: var(--bg-0, #0e0e10) }` and the splash.
  - Do not create the window hidden: a failed bundle would then leave an invisible window.
- **Minimum size.**
  - The UI needs a client area of at least **1100 × 680**.
  - On Windows, without decorations and with `shadow: true`, the minimum applies to the outer window, while the invisible resize borders stay outside the web view. That is 8px on the left, right and bottom at 100 % scaling, plus 1px at the top on Windows 11.
  - So the configured minimum is **1116 × 689**.
- **Frameless title bar.**
  - The 48px `.appbar` is the title bar, marked with `data-tauri-drag-region`, and so is its `.brand` block. The children of `.brand` get `pointer-events: none`.
  - Double-clicking maximizes.
  - Three `.window-control` buttons, 46px wide each: `Minimizar`, `Maximizar` (which becomes `Restaurar`), and `Cerrar`. Close hovers in ink, not red.
  - Capabilities: `core:window:allow-start-dragging`, `allow-internal-toggle-maximize`, `allow-minimize`, `allow-toggle-maximize` and `allow-close`.
  - The Windows 11 snap-layout flyout is not available. `Win + Z`, `Win + arrows` and edge snapping still work.
- **Inline splash.**
  - The splash is static HTML in `index.html`: the logo paths and three dots.
  - Its critical CSS is inline, with fallback values, so it paints before any stylesheet or script. The CSP needs `style-src 'unsafe-inline'`, not inline scripts.
  - It is a drag region, and the window controls stay above it. The app behind is `inert` while it shows.
  - It fades out on the first meaningful render, with no minimum time and a safety timeout of about 5 s.
  - With reduced motion, the dots are static.

```html
<div class="splash" id="splash" data-tauri-drag-region="deep">
  <svg class="splash__logo" viewBox="0 0 24 24" aria-hidden="true"><!-- the app's mark paths --></svg>
  <div class="splash__body">
    <span class="dots" role="status" aria-label="Cargando"><span></span><span></span><span></span></span>
  </div>
</div>
```

- **Scrolling.** The document never scrolls; regions do. Each app sets `html, body { height: 100%; overflow: hidden }`.
- **Reload keys.** A production build may ignore F5, Ctrl+R and Ctrl+Shift+R where a reload would lose work in progress.

## 10. Icons

- **Grid and stroke.** A 24 grid, 1.5 stroke, round caps and joins, no fill. The logo, play and stop are the filled exceptions.
- **Sizes.** 16px in controls, 15 in small ones, 14 in window controls.
- **Window controls.** Their stroke is 1.4 so it matches the thin native caption glyphs; this is the one exception to the 1.5 rule.
- **In React.** Use lucide with `strokeWidth={1.5}`: `<Download size={16} strokeWidth={1.5} />`.
- **In vanilla.** Use inline SVG with the `.icon` class.

| ChronoRift `ICONS` | lucide | Note |
| --- | --- | --- |
| `search` | `Search` | |
| `refresh` | `RotateCw` | A single clockwise arrow, not `RefreshCw` |
| `folder` | `Folder` | |
| `file` | `File` | |
| `exe` | `FileUp` | A file with an up arrow; ChronoRift uses it for "Elegir ejecutable" |
| `copy` | `Copy` | |
| `download` | `Download` | |
| `close` | `X` | |
| `external` | `ExternalLink` | |
| `minimize` | `Minus` | Window control |
| `maximize` | `Square` | Window control; lucide's square is larger (18 vs 13 units) |
| `restore` | `Copy`, mirrored (`scale(-1, 1)`) | No exact lucide glyph; Windows puts the back square top-right |
| `check` | `Check` | |
| `chevron` | `ChevronDown` | Select fields |
| `drive` | `HardDrive` | |
| `user` | `User` | |
| `play` | `Play` with `fill="currentColor"` | Filled |
| `stop` | `Square` with `fill="currentColor"`, drawn smaller | Filled; ChronoRift's is 12 units, lucide's 18 |
| `logo` | — | The app's mark; never from an icon set (see `logo/`) |

## 11. Logo

- **Monochrome ink, always.** `#EDEDEF` on dark, `#0E0E10` on light, never brand.
- **Construction.** The 24-grid construction rules, the app tile and the mandatory checks at 16, 20, 24 and 32 px are in [`logo/construction.md`](logo/construction.md).
- **Files.**
  - `logo/template.svg` is the drawing template.
  - `logo/chronorift.svg` is the reference example.

## 12. Fonts and licenses

- **The fonts.** Geist and Geist Mono are Copyright 2024 The Geist Project Authors
  (https://github.com/vercel/geist-font), licensed under the SIL Open Font License, Version 1.1
  (http://scripts.sil.org/OFL).
- **The kit doesn't bundle them.** Each app installs `@fontsource-variable/geist` and `@fontsource-variable/geist-mono` (peer dependencies, 5.3.x), imports them once, and ships them inside the app.
- **Each app reproduces the OFL notice and full license text in its own `THIRD_PARTY_NOTICES.md`.** ChronoRift's already does.
- **Fallback fonts.**
  - Sans: Segoe UI Variable Text, Segoe UI, system-ui.
  - Mono: Cascadia Mono, Consolas, ui-monospace.

## 13. Names shared with Tailwind and shadcn

A Tailwind app has to resolve these before mapping the kit in `tailwind.css`.

| Kit name | Also used by | What to do |
| --- | --- | --- |
| `--radius` | shadcn's base radius (FairTranslator: 0.5rem), from which `@theme inline` derives `--radius-sm/md/lg/xl` | The kit's value is an absolute 7px. Let one side win and set the shadcn scale explicitly instead of deriving it. |
| `--radius-sm`, `--radius-md`, `--radius-lg` | Tailwind's `rounded-*` theme names | Kit values are absolute: 4, 6 and 10. |
| `--font-sans`, `--font-mono` | Tailwind's font theme | Same meaning; the kit's value is the one wanted. |
| `--success`, `--warning`, `--info` | FairTranslator's shadcn-style tokens, each with a `-foreground` pair | In the kit these are text colors on dark surfaces, with `-bg` and `-line` variants and no `-foreground`. |
| `--danger` | shadcn's `--destructive` | Map `--destructive` to `var(--danger)`. |
| `--shadow-drawer` | Tailwind's `--shadow-*` names, which hold full shadows | In the kit it is a color. |
| `--brand*` | nothing | Named to avoid shadcn's `--primary` and `--accent`. |
| Scrollbars | `scrollbar-color` and `scrollbar-width` in FairTranslator's base layer | Remove them: Chromium 121 and later ignore `::-webkit-scrollbar` once they are set. |
| Cascade | Tailwind's layers | Import `base.css` with `layer(base)` and `components.css` with `layer(components)`. Keep `tokens.css` unlayered and before the app's own `:root` overrides. |

## 14. Protocol for both sessions

- **Edit the kit, never a vendored copy.** The ChronoRift and FairTranslator sessions both edit this repo. Apps never edit their vendored copy.
- **Announce, then report.**
  - Before editing, announce what will change: files, tokens and classes.
  - After committing, post **"listo"** with the short hash.
- **Small commits.** Each commit adds a line to `CHANGELOG.md`.
- **The contract** is the token names and roles, the class names, the file names, and the CLI of `sync.mjs` and `check.mjs`.
  - Contract changes (renames, removals, changes of meaning) are agreed with the other session first. While in 0.x they bump the minor version, for example 0.1.0 to 0.2.0.
  - Additions (a new token, class or spec) are announced and bump the patch version.
- **Commit identity.** Use `git -c user.name="Diego" -c user.email="202112436@urp.edu.pe" commit`.
  - No `Co-Authored-By` and no "Generated with" lines.
  - No changes to git config.
- **Releases.** Tag each release `vX.Y.Z`. Each app then runs `sync.mjs` and commits the vendored copy with its `KIT_VERSION`.
