import {
  normalizeEntryStatus,
  normalizeProjectEntryUpdate,
} from "@/lib/translationState";
import {
  fileStats,
  projectStats,
  isTranslatedEntry,
} from "@/lib/translationMetrics";
import type { TranslationEntry } from "@/types/domain";

describe("translation state normalization", () => {
  const baseEntry: TranslationEntry = {
    id: "Map001:text:1",
    fileContext: "Map001 > Event 1",
    original: "Hello",
    translation: "",
    status: "untranslated",
    notes: "",
  };

  it("forces untranslated status when translation is blank", () => {
    expect(normalizeEntryStatus("   ", "final")).toBe("untranslated");
    expect(
      normalizeProjectEntryUpdate({
        filePath: "Data/Map001.json",
        entryId: "Map001:text:1",
        translation: "",
        status: "reviewed",
      }),
    ).toEqual({
      filePath: "Data/Map001.json",
      entryId: "Map001:text:1",
      translation: "",
      status: "untranslated",
    });
  });

  it("does not count blank translations as translated even if status drifted", () => {
    const driftedEntry: TranslationEntry = {
      ...baseEntry,
      status: "reviewed",
      translation: "   ",
    };

    expect(isTranslatedEntry(driftedEntry)).toBe(false);
  });

  it("keeps non-empty translations translated when status is not untranslated", () => {
    const translatedEntry: TranslationEntry = {
      ...baseEntry,
      translation: "Hola",
      status: "draft",
    };

    expect(isTranslatedEntry(translatedEntry)).toBe(true);
  });

  it("counts progress per file without treating blank originals as work", () => {
    const entries: TranslationEntry[] = [
      baseEntry,
      { ...baseEntry, id: "Map001:text:2", translation: "Hola", status: "draft" },
      { ...baseEntry, id: "Map001:text:3", original: "   ", translation: "", status: "untranslated" },
      { ...baseEntry, id: "Map001:text:4", translation: "   ", status: "reviewed" },
    ];

    const stats = fileStats(entries);
    expect(stats).toEqual({
      total: 3,
      translated: 1,
      pending: 2,
      byStatus: { untranslated: 1, draft: 1, reviewed: 1, final: 0 },
    });
    // Mismo array, mismo resultado en caché; un array nuevo (una edición) se recalcula.
    expect(fileStats(entries)).toBe(stats);
    expect(fileStats([...entries])).not.toBe(stats);
    expect(projectStats([{ entries }, { entries }]).pending).toBe(4);
  });
});
