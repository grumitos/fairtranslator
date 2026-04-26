import type { EntryStatus, Project } from "@/types/domain";

const statuses: EntryStatus[] = ["untranslated", "draft", "reviewed", "final"];

function demoFileName(index: number): string {
  const base = ["System.json", "Actors.json", "Items.json", "CommonEvents.json"];
  return index < base.length ? `data/${base[index]}` : `data/Map${String(index - 3).padStart(3, "0")}.json`;
}

export function createVisualDemoProject(): Project {
  const now = new Date().toISOString();

  return {
    id: "visual-demo",
    name: "Visual QA Dense RPG Project",
    sourceLang: "ja",
    targetLang: "es",
    engine: "mz",
    gameDir: "D:/Demo/RPG",
    glossary: [],
    styleGuide: ["Keep combat commands short.", "Preserve RPG Maker control codes."],
    createdAt: now,
    updatedAt: now,
    files: Array.from({ length: 14 }, (_, fileIndex) => {
      const relativePath = demoFileName(fileIndex);

      return {
        relativePath,
        entries: Array.from({ length: 18 }, (_, entryIndex) => {
          const status = statuses[(fileIndex + entryIndex) % statuses.length];
          return {
            id: `${fileIndex}-${entryIndex}`,
            fileContext: `${relativePath} > Event ${entryIndex + 1}`,
            original: `Original line ${entryIndex + 1}: 勇者は扉を開けた。\\V[${entryIndex}] Keep tags stable.`,
            translation: status === "untranslated"
              ? ""
              : `Linea traducida ${entryIndex + 1}: El heroe abrio la puerta.`,
            status,
            notes: entryIndex % 5 === 0 ? "Check tone consistency." : "",
          };
        }),
      };
    }),
  };
}
