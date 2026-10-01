import type { TranslationFile } from "@/types/domain";

export type FileGroupKey = "system" | "database" | "events" | "maps";

export function fileStem(relativePath: string): string {
  const fileName = relativePath.split("/").pop() ?? relativePath;
  return fileName.replace(/\.[^.]+$/, "");
}

export function fileGroupKey(relativePath: string): FileGroupKey {
  const name = fileStem(relativePath);
  if (name === "System") return "system";
  if (name === "CommonEvents" || name === "Troops") return "events";
  if (/^Map\d+$/.test(name)) return "maps";
  return "database";
}

const GROUP_ORDER: FileGroupKey[] = ["system", "database", "events", "maps"];

// Agrupa los archivos del proyecto como en el editor de RPG Maker: sistema, base de datos, eventos y mapas.
export function groupFiles(files: TranslationFile[]): { key: FileGroupKey; files: TranslationFile[] }[] {
  const buckets = new Map<FileGroupKey, TranslationFile[]>();
  for (const file of files) {
    const key = fileGroupKey(file.relativePath);
    const bucket = buckets.get(key);
    if (bucket) bucket.push(file);
    else buckets.set(key, [file]);
  }
  return GROUP_ORDER.filter((key) => buckets.has(key)).map((key) => ({ key, files: buckets.get(key)! }));
}
