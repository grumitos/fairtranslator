import type { EntryStatus, ProjectEntryUpdate } from "@/types/domain";

export function hasTranslationContent(translation: string): boolean {
  return translation.trim() !== "";
}

/** Estado tras editar el texto: una entrada sin traducir que recibe texto pasa a borrador; las demás lo conservan. */
export function statusAfterEdit(current: EntryStatus, translation: string): EntryStatus {
  return current === "untranslated" && hasTranslationContent(translation) ? "draft" : current;
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
