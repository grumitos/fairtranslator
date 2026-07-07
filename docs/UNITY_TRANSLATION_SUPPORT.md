# Unity Translation Support Plan

FairTranslator does not yet ship first-class Unity support. This document defines
the safe support boundary before adding `Engine::Unity` to the product.

## Sources

Unity references used for this plan:

- Scripting backends: <https://docs.unity3d.com/Manual/scripting-backends.html>
- IL2CPP: <https://docs.unity3d.com/6000.5/Documentation/Manual/scripting-backends-il2cpp.html>
- Reserved folders, including `Resources` and `StreamingAssets`: <https://docs.unity3d.com/6000.5/Documentation/Manual/SpecialFolders.html>
- Streaming Assets platform behavior: <https://docs.unity3d.com/2020.1/Documentation/Manual/StreamingAssets.html>
- AssetBundles: <https://docs.unity3d.com/6000.4/Documentation/Manual/AssetBundlesIntro.html>
- Addressables build artifacts/catalogs: <https://docs.unity3d.com/Packages/com.unity.addressables%401.20/manual/BuildArtifacts.html>
- Localization String Tables: <https://docs.unity3d.com/Packages/com.unity.localization%401.5/api/UnityEngine.Localization.Tables.StringTable.html>
- CSV import/export for String Tables: <https://docs.unity3d.com/6000.4/Documentation/Manual/best-practice-guides/ui-toolkit-for-advanced-unity-developers/localization.html>

## Unity Game Types

Classify Unity games by these axes, not by a single version number:

| Axis | Cases | Translation impact |
| --- | --- | --- |
| Scripting backend | Mono, IL2CPP | Mono may expose `Managed/Assembly-CSharp.dll`; IL2CPP compiles ahead-of-time to native code and must not be treated like a DLL string patch. |
| Platform | Windows, Linux, macOS, Android, iOS, WebGL | Folder layout and writable/replaceable files vary. Start with desktop builds only. |
| Content packaging | `resources.assets`, `sharedassets*.assets`, AssetBundles, Addressables, `StreamingAssets`, loose files | Text may live in multiple containers. Export must patch a copied game tree only. |
| Text format | TextAsset JSON/CSV/YAML/TXT/XML, custom script DSL, Unity Localization String Tables, scene/prefab serialized text, managed `ldstr`, images | TextAsset is the first viable editable target. Managed strings and scenes need stricter context. Images are out of scope for parser support. |
| Delivery | local files, DLC, remote Addressables/catalogs, cached bundles | Local files are viable. Remote/cached content is inventory-only until there is a repeatable download/update path. |
| Protection | plain, obfuscated, encrypted, packed | Plain is viable. Obfuscated/encrypted should be reported, not guessed around. |

## What To Support First

Phase 1 should support a narrow, repeatable Unity slice:

- Desktop Unity builds with a detectable `*_Data` folder.
- Copy original game to the export folder before patching.
- Detect `globalgamemanagers`, `resources.assets`, `sharedassets*.assets`,
  `StreamingAssets`, `Managed/Assembly-CSharp.dll`, `GameAssembly.dll`, and
  `il2cpp_data/Metadata/global-metadata.dat`.
- Extract editable entries from TextAssets in local AssetBundles/Addressables
  when their content decodes as clear UTF-8/UTF-16 text.
- Produce an editor/AI-friendly work file with stable IDs, context, source text
  as read-only reference and a separate translation field. Do not make users or
  agents rewrite the source text inline.
- Preserve file/container metadata enough to write back to the same copied
  asset.
- For local Addressables bundles, update the catalog CRC/hash after a bundle is
  rewritten; otherwise Unity rejects the bundle before gameplay scripts load.
- Reopen patched assets and verify translated strings are present.

This is the Unity equivalent of the existing RPG Maker parser/exporter contract:
detect, parse visible text, translate, copy, patch copy, validate.

## Read-Only Inventory First

These should be detected and reported before any write support:

- Mono managed DLL strings. They can contain visible UI, but identical `ldstr`
  values may be shared by visible text and internal keys.
- IL2CPP metadata/native strings. Treat as unsupported for patching.
- Unity Localization Package String Tables inside assets/bundles.
- Scenes, prefabs, and generic serialized assets.
- Remote Addressables catalogs and bundles.

## Out Of Scope For The First Product Slice

- Generic `Assembly-CSharp.dll` patching.
- IL2CPP binary patching.
- Scene/prefab editing without a validated Unity asset writer.
- WebGL/Android/iOS patching.
- Encrypted, obfuscated, or custom-packed assets.
- Image/sprite text replacement.

## Lessons From RJ01625259

What worked:

- Addressables bundle `novelscripts_assets_all_*.bundle` contained 21 `TextAsset`
  script files.
- Extraction produced 1,546 script rows and 213 assembly-string rows.
- UnityPy could patch a copied TextAsset bundle.
- A Mono.Cecil helper could patch selected `Assembly-CSharp.dll` strings.
- A script-only reinjection path can patch the bundle and update
  `catalog.bin`/`catalog.hash` with the Unity payload CRC.
- A clean TSV work file with only script rows avoids assembly/internal-key noise
  for AI or manual editing.

What must not become product debt:

- Hardcoded bundle path/name.
- IDs based only on script line numbers.
- Patching all matching DLL strings without method/class context.
- Exporting raw technical inventories as the primary translation work surface.
- Modifying the original game instead of a copied output.
- Depending on Python/UnityPy/C# helper as an undocumented runtime requirement.

## Agent Review Result

A read-only gpt-5.5 xhigh review of the RJ01625259 prototype agreed with the
same boundary: ship Unity only after a copy-first TextAsset
AssetBundle/Addressables path exists, and keep Mono DLL strings as inventory
until method/class context and safe replacement rules are implemented. The
prototype is useful as a fixture, not as product architecture.

## Recommended Architecture

Do not add `Engine::Unity` until the first parser/exporter slice exists.

Minimal future shape:

```text
src-tauri/src/parser/unity.rs
  detect_unity_game_dir()
  parse_unity_game() -> TranslationFile entries for supported TextAssets

src-tauri/src/services/exporter.rs
  UnityExporter copies source tree and patches only supported copied assets

src-tauri/src/models/mod.rs
  Engine::Unity only when parser/exporter are both wired and tested
```

Keep Unity support behind the same `Project -> TranslationFile -> TranslationEntry`
contract. Add metadata only when the exporter truly needs it; avoid a generic
plugin system until at least two real Unity fixtures require the same extension
point.

## Minimum Fixtures

Before declaring Unity support:

- One Mono desktop build with clear TextAsset files.
- One IL2CPP desktop build with clear TextAsset files and no DLL patching.
- One Addressables local bundle fixture.
- One negative fixture with encrypted/unsupported content that reports a clear
  unsupported reason.

Each fixture must verify import, translation apply, export copy, reopen/readback,
and source-folder immutability.
