import {
  normalizeEntryStatus,
  normalizeProjectEntryUpdate,
} from "@/lib/translationState";
import {
  isPendingTranslationEntry,
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

  it("treats blank translations as pending even if status drifted", () => {
    const driftedEntry: TranslationEntry = {
      ...baseEntry,
      status: "reviewed",
      translation: "   ",
    };

    expect(isPendingTranslationEntry(driftedEntry)).toBe(true);
    expect(isTranslatedEntry(driftedEntry)).toBe(false);
  });

  it("keeps non-empty translations translated when status is not untranslated", () => {
    const translatedEntry: TranslationEntry = {
      ...baseEntry,
      translation: "Hola",
      status: "draft",
    };

    expect(isPendingTranslationEntry(translatedEntry)).toBe(false);
    expect(isTranslatedEntry(translatedEntry)).toBe(true);
  });
});
