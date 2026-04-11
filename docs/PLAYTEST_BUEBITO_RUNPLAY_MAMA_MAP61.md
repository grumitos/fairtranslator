# Buebito Runplay Detail - Mama Route on Map 61

## Scope
- Date: `2026-04-11`
- Export: `D:\FairTranslatorRuns\buebito-codex-20260411-spark-low\export-es`
- Runtime URL: `http://127.0.0.1:8770/index.html?fresh=8`
- Flow origin: map `63` recall hub, event `キャラ_ママ`
- Capture method: real browser runtime plus automation-assisted `window.Input.virtualClick('ok')` when the canvas would not take stable keyboard focus

## Entry Frame Before the Scene
- Screenshot: `buebito-menu-hold-before-continue.png`
- The first visible checkpoint before re-entering the dialogue is not a menu but the blue-tinted hub itself. The whole screen is washed in a cold violet-blue layer that flattens most warm colors and makes the silhouettes read first.
- Mama is the tall adult-coded woman on the right. She has long dark hair, a soft rounded face, and a visibly fuller chest and hips than the other figures. Her blue short-sleeve top and pale skirt read clearly even through the night tint.
- Yuu stands near her as a much smaller, slimmer figure in a blue shirt and dark shorts. The height gap is immediate: his head sits well below her shoulder line, so the scene reads as adult woman plus child-sized protagonist before any dialogue appears.
- A second adult-coded woman with brown hair stands apart on the left in a sleeveless dark-red top and dark skirt. A fourth character is only partially visible at the bottom edge as a cropped head and hair silhouette.
- HUD remains stable: yellow HP bar at top left, moon/time icon below it, and the rounded hamburger menu at top right. No visible text is on screen in this freeze frame.

## Black-Screen Dialogue Phase
- Screenshots:
  - `buebito-hub-pre-interaction-2.png`
  - `buebito-mama-scene-step2.png`
  - `buebito-mama-scene-step3-choice.png`
  - `buebito-mama-scene-step4-choice-list.png`
  - `buebito-mama-scene-step5-cg.png`
  - `buebito-mama-scene-step6.png`
  - `buebito-mama-scene-step7.png`
  - `buebito-mama-scene-step8.png`
- Once the Mama event opens, the image collapses almost completely into black. Only three layers remain visible:
  - the yellow HP bar and moon icon at top left,
  - the black field occupying the entire play area,
  - the bottom message window with portrait, name tag, and text.
- This staging is important for QA because there is no background art, no map clutter, and no animation to distract from the text. Every translation choice lands directly on the player.
- Yuu opens the scene excitedly. His portrait is youthful and soft-faced, with brown hair and a blue shirt; the face graphic is clean and centered, and the Spanish fits in the box without clipping.
- Mama answers through her face portrait only. Her portrait shows long dark hair, pinkish cheeks, and a small smile. Since her full body disappears during this phase, the scene relies entirely on the emotional reading of her expression and the text rhythm.
- The black-screen portion lasts for several dialogue boxes in a row. Visually it feels less like a location and more like a suspended memory or fantasy prelude. It is readable, but also exposes every literal line very harshly because there is no environmental texture softening the writing.

## Visible Text and Translation Read
- Cleanly visible and structurally correct:
  - `Ufufu♡ Que Yuu-kun me enseñe algo...`
  - `Mama se siente tan feliz♡`
  - `Entonces, Yuu-kun♡ ¿Me enseñarás?`
  - `¿Qué debería hacer Mama?`
  - `Tienes razón, lo siento♡`
  - `Ufufu♡ Yuu-kun me regañó♡`
- Translation quality issues in this block:
  - `Mama se siente tan feliz♡` is grammatically understandable but sounds like direct carryover from Japanese self-reference instead of natural Spanish dialogue.
  - `¿Qué debería hacer Mama?` has the same issue. It is readable, but the voice sounds unnatural because Mama keeps naming herself instead of speaking directly.
  - `Mama no me importa ni de esta ni de la otra♡` is the weakest line in this segment. It does not overflow, but the Spanish reads malformed and unclear.
  - Several Yuu lines in this sequence shift into a harsher or stiffer register than the surrounding text. The structure still fits inside the box, but the tone feels less localized and more literal.
- Technical integrity:
  - no broken control codes,
  - no placeholder corruption,
  - no overflow,
  - no broken line wrapping,
  - hearts `♡` display correctly.

## CG Reveal and Full-Body Composition
- Screenshots:
  - `buebito-mama-scene-post-dialogue.png`
  - `buebito-mama-scene-step9-after-choice.png`
- After the black-screen buildup, the scene finally opens into a full bedroom CG on map `61`.
- The room is a small Japanese-style interior under deep blue-purple night lighting. The palette is cool and subdued, and most objects pick up the same indigo cast.
- Left side:
  - a large shoji window fills the upper wall,
  - a red randoseru-style backpack sits on the floor,
  - an open workbook or magazine lies beside scattered writing tools,
  - a small blue trash bin stands near the bed.
- Center/back:
  - a dark two-seat couch sits against the wall,
  - the mattress/futon occupies the middle of the room with pale blue bedding and a white pillow,
  - the characters are fully visible on the futon.
- Right side:
  - a low dark side table holds a box-shaped item,
  - a round dark floor cushion sits nearby.
- Character composition:
  - Mama is positioned lower and forward on the bedding, propped on her elbows. Her long dark hair falls toward the pillow side, and her body reads as distinctly larger and more mature than Yuu's. Even in the compressed pixel-art style, her adult-coded proportions are obvious.
  - Yuu is behind and above her, in a blue shirt, with a much smaller frame and narrower shoulders. The size contrast is strong: he looks short and slight next to Mama's taller, fuller build.
  - This is the first point in the sequence where both bodies can be judged together rather than through portraits alone.
- UI overlay:
  - the choice window appears on the right side of the bed with `Con fuerza.` and `No puedo contenerme.`
  - after selecting the default highlighted branch `Con fuerza.`, the message window returns over the CG.
  - the bottom message box covers part of the bedding and lower bodies, but it does not hide the key pose or room context.

## Translation Read on the CG Phase
- The CG phase remains technically stable: no clipping, no placeholder problems, no broken menu labels.
- The selected branch label `Con fuerza.` is understandable and short enough for the choice box.
- The following Yuu line after choosing it also fits well on screen, but the voice remains more literal than polished. The scene is functional in Spanish, not broken, but it does not yet read with a fully natural cadence.

## QA Assessment
- Visual success:
  - the scene transitions correctly from hub to black-screen memory mode to full bedroom CG,
  - portraits, HUD, and choice box all render correctly,
  - body-size contrast between Mama and Yuu is visually clear in the standing hub shot and again in the map `61` CG,
  - no runtime errors appeared during this route.
- Localization concerns:
  - black-screen lines are the weakest part because they expose unnatural self-reference very clearly,
  - at least one Mama line is semantically awkward enough to justify follow-up triage,
  - the scene is readable and functional, but not yet tonally smooth in Spanish.
- Current state at pause:
  - map `61`
  - bedroom CG active
  - branch selected: `Con fuerza.`
  - next visible text already continuing after the branch selection
