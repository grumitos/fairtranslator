# Unity Loose JSON v0 Support Boundary

FairTranslator has one experimental Unity compatibility profile: **Unity
(Loose JSON v0)**. It handles a deliberately narrow Windows Mono layout with
one loose localization JSON file. This is not generic Unity support.

## Sources

Unity references used for this support boundary:

- Scripting backends: <https://docs.unity3d.com/Manual/scripting-backends.html>
- IL2CPP: <https://docs.unity3d.com/6000.5/Documentation/Manual/scripting-backends-il2cpp.html>
- Reserved folders, including `Resources` and `StreamingAssets`: <https://docs.unity3d.com/6000.5/Documentation/Manual/SpecialFolders.html>
- Streaming Assets platform behavior: <https://docs.unity3d.com/2020.1/Documentation/Manual/StreamingAssets.html>
- AssetBundles: <https://docs.unity3d.com/6000.4/Documentation/Manual/AssetBundlesIntro.html>
- Addressables build artifacts/catalogs: <https://docs.unity3d.com/Packages/com.unity.addressables%401.20/manual/BuildArtifacts.html>
- Localization String Tables: <https://docs.unity3d.com/Packages/com.unity.localization%401.5/api/UnityEngine.Localization.Tables.StringTable.html>
- CSV import/export for String Tables: <https://docs.unity3d.com/6000.4/Documentation/Manual/best-practice-guides/ui-toolkit-for-advanced-unity-developers/localization.html>

## Current Experimental Profile

A game is accepted only when all of these conditions hold:

- It is a Windows Unity Mono build.
- The selected game root contains `UnityPlayer.dll` and exactly one root-level
  `*_Data` directory.
- That data directory contains `globalgamemanagers`,
  `Managed/Assembly-CSharp.dll`, and `StreamingAssets`.
- `StreamingAssets` contains exactly one `.json` candidate whose path or
  filename starts with `localization`, `locales`, `lang`, `i18n`, or
  `strings` (optionally followed by `-`, `_`, or `.`).
- The candidate is at most 8 MiB, is UTF-8 with an optional UTF-8 BOM, and its
  top level is a flat object whose keys and values are strings.
- Keys are unique, non-empty stable IDs without control characters. Blank
  values are retained in the file but are not offered for translation; at
  least one value must contain translatable text.

The bounded `StreamingAssets` scan stops at 8 directory levels, 2,048
directories, or 50,000 entries. Symbolic links and Windows reparse points are
rejected instead of being followed outside the selected game.

Entries use JSON Pointer-escaped keys as stable IDs. The Mono DLL is only a
layout marker; FairTranslator never reads or patches its strings.

## Export Safety

Unity export follows the same immutable-source rule as the RPG Maker exporters:

1. Reparse the source and reject export if any recognized original text drifted
   after import.
2. Copy the game tree to a separate output directory.
3. Update only the recognized JSON values in that copy, and only when their
   current values still match the project's originals.
4. Preserve an existing UTF-8 BOM.
5. Reopen the copied game and verify every applied translation.

If copy, patch, or readback verification fails, the incomplete output is
removed. The source game folder is never modified.

## Explicitly Unsupported

Unity Loose JSON v0 does **not** support:

- IL2CPP builds, including games with `GameAssembly.dll` or
  `il2cpp_data/Metadata/global-metadata.dat`.
- AssetBundles, Addressables bundles or catalogs, `resources.assets`,
  `sharedassets*.assets`, scenes, prefabs, or other Unity serialized assets.
- `Assembly-CSharp.dll` string extraction or patching.
- Unity Localization String Tables stored inside assets or bundles.
- Nested JSON objects, arrays, CSV, YAML, XML, TXT, or custom script formats.
- Linux, macOS, Android, iOS, WebGL, consoles, or any platform beyond Windows.
- Remote content, DLC, caches, encrypted or obfuscated data, and image text.

Detection of a Unity executable or `*_Data` directory alone does not imply
support. Product copy must use the full name **Unity (Loose JSON v0)** and keep
the experimental label until the profile has real-game validation.

## Why The Boundary Is Narrow

`StreamingAssets` loose files can be copied and rewritten without pretending
that arbitrary Unity containers share one safe format. AssetBundles,
Addressables, serialized objects, managed strings, and IL2CPP metadata each
need different identity, writeback, and integrity rules. Guessing across those
boundaries risks corrupting the exported game or translating internal keys.

The existing `Project -> TranslationFile -> TranslationEntry` contract is
enough for this profile. A generic plugin system is intentionally deferred
until at least two proven engine formats need the same extension point.

## Research Notes From RJ01625259

The RJ01625259 prototype remains useful research, not a claim of product
support:

- Its Addressables bundle exposed 21 TextAsset scripts and produced 1,546
  script rows.
- UnityPy could rewrite a copied TextAsset bundle, and a separate helper could
  patch selected Mono DLL strings.
- Rewritten Addressables content also required catalog CRC/hash updates.
- A focused script work file was substantially safer than exposing raw
  assembly/internal-key inventories.

Those results do not belong in Loose JSON v0. Hardcoded bundle paths, line-only
IDs, broad DLL replacement, source-folder mutation, and undocumented
Python/UnityPy/C# runtime requirements remain unacceptable product shortcuts.

## Roadmap Beyond v0

Before promoting Loose JSON v0 out of experimental status:

- Validate import, export, readback, and source immutability on multiple real
  Windows Mono games using this exact loose-JSON layout.
- Keep positive fixtures plus negative IL2CPP, ambiguous-candidate, nested-JSON,
  oversized-file, deep-tree, and symbolic-link fixtures.
- Document recovery guidance for games rejected by the profile.

Any broader Unity work should land as a separately named, tested profile:

1. Inventory unsupported local formats without promising write support.
2. Add a container reader/writer only after real fixtures prove stable object
   identity and copy-first round trips.
3. Treat AssetBundles and Addressables catalog updates as their own integrity
   boundary.
4. Keep managed DLL and IL2CPP patching out of scope until each has a dedicated
   safety design and verification strategy.

No roadmap item broadens the current support claim until its parser, exporter,
fixtures, failure cleanup, and readback checks are shipped together.
