import type { EntryStatus, TranslationEntry } from "@/types/domain";
import type { EntryWithPath } from "@/components/editor/translation-grid/types";

// Las entradas sin cambios conservan su referencia tras cada edición (immer comparte estructura), así
// que la fila del grid y su texto de búsqueda se reutilizan y solo se reconstruye la entrada editada.
const gridEntryCache = new WeakMap<TranslationEntry, EntryWithPath>();

function buildSearchText(entry: TranslationEntry): string {
  return [entry.original, entry.translation, entry.fileContext, entry.id].join("\n").toLowerCase();
}

export function toGridEntry(entry: TranslationEntry, filePath: string): EntryWithPath {
  let cached = gridEntryCache.get(entry);
  if (!cached || cached._filePath !== filePath) {
    cached = { ...entry, _filePath: filePath, _searchText: buildSearchText(entry) };
    gridEntryCache.set(entry, cached);
  }
  return cached;
}

/**
 * Mismo filtro que la tabla (estado y texto de búsqueda), para que Buscar y reemplazar cuente y
 * reemplace solo lo que el usuario ve. `query` llega ya en minúsculas.
 */
export function isVisibleInGrid(entry: EntryWithPath, statusFilter: EntryStatus | null, query: string): boolean {
  if (statusFilter && entry.status !== statusFilter) return false;
  return !query || (entry._searchText ?? buildSearchText(entry)).includes(query);
}
