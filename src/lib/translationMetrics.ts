import type { TranslationEntry } from "@/types/domain";

export function isTranslatableEntry(entry: TranslationEntry): boolean {
  return entry.original.trim() !== "";
}

export function isPendingTranslationEntry(entry: TranslationEntry): boolean {
  return isTranslatableEntry(entry) && entry.status === "untranslated";
}

export function isTranslatedEntry(entry: TranslationEntry): boolean {
  return isTranslatableEntry(entry) && entry.status !== "untranslated";
}
