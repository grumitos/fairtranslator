import type { TranslationEntry } from "@/types/domain";
import { hasTranslationContent } from "@/lib/translationState";

export function isTranslatableEntry(entry: TranslationEntry): boolean {
  return entry.original.trim() !== "";
}

export function isPendingTranslationEntry(entry: TranslationEntry): boolean {
  return (
    isTranslatableEntry(entry)
    && (entry.status === "untranslated" || !hasTranslationContent(entry.translation))
  );
}

export function isTranslatedEntry(entry: TranslationEntry): boolean {
  return (
    isTranslatableEntry(entry)
    && entry.status !== "untranslated"
    && hasTranslationContent(entry.translation)
  );
}
