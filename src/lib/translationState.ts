import type { EntryStatus, ProjectEntryUpdate } from "@/types/domain";

export function hasTranslationContent(translation: string): boolean {
  return translation.trim() !== "";
}

export function normalizeEntryStatus(
  translation: string,
  status: EntryStatus,
): EntryStatus {
  return hasTranslationContent(translation) ? status : "untranslated";
}

export function normalizeProjectEntryUpdate(
  update: ProjectEntryUpdate,
): ProjectEntryUpdate {
  return {
    ...update,
    status: normalizeEntryStatus(update.translation, update.status),
  };
}
