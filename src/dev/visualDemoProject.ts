import { usePanelStore } from "@/components/layout/panelStore";
import { useCliUpdateStore } from "@/stores/cliUpdateStore";
import { useSettingsStore } from "@/stores/settingsStore";
import type {
  EntryStatus,
  ExportResult,
  GameInspection,
  Project,
  ProviderType,
  TextFitReport,
  TranslationEntry,
} from "@/types/domain";

/*
 * Proyecto de muestra para revisar la interfaz sin Tauri (`?visualDemo=1` en `npm run dev`).
 * Diálogos de RPG genéricos con códigos de control reales de RPG Maker y los cuatro estados, para
 * que las capturas a 1100×680 y 1360×840 muestren casos representativos.
 */

const LINES: [string, string, string][] = [
  ["Posadera", "「いらっしゃい！\\N[1]、久しぶりね。」", "¡Bienvenido, \\N[1]! Cuánto tiempo."],
  ["Posadera", "一晩\\V[3]ゴールドだけど、泊まっていく？", "Son \\V[3] de oro por noche. ¿Te quedas?"],
  ["Posadera", "ゆっくり休んでね。", "Descansa bien."],
  ["Viajero", "この先の森には\\C[2]霧の魔物\\C[0]が出るらしい。", "Dicen que en el bosque aparece el \\C[2]monstruo de la niebla\\C[0]."],
  ["Viajero", "夜は絶対に出歩くなよ。", "Ni se te ocurra salir de noche."],
  ["Niña", "ねえ、お話して！", "¡Oye, cuéntame una historia!"],
  ["Niña", "……もう寝る時間？", "…¿Ya es hora de dormir?"],
  ["Cofre", "\\I[64]\\C[3]回復薬\\C[0]を手に入れた！", "¡Obtuviste \\I[64]\\C[3]Poción\\C[0]!"],
  ["Letrero", "宿屋「月の雫」", "Posada «Gota de Luna»"],
  ["Borracho", "ひっく……勇者だって？笑わせるな。", "Hic… ¿Un héroe? No me hagas reír."],
  ["Borracho", "\\{何か用か！\\}", "\\{¿Qué quieres?\\}"],
  ["Cocinero", "今日のスープは\\V[5]種類の野菜入りだ。", "La sopa de hoy lleva \\V[5] tipos de verdura."],
  ["Cocinero", "また来いよ。", "Vuelve pronto."],
  ["Gato", "ニャー。", "Miau."],
];

// Archivo, entradas y cuántas llevan traducción. Uno completo y uno sin empezar, como en un proyecto real.
const FILES: [string, number, number][] = [
  ["System", 28, 22],
  ["Actors", 12, 12],
  ["Items", 18, 14],
  ["CommonEvents", 24, 13],
  ["Map001", 16, 16],
  ["Map002", 22, 9],
  ["Map003", 26, 16],
  ["Map004", 14, 0],
];

const TRANSLATED_STATUSES: EntryStatus[] = ["final", "reviewed", "draft", "final", "reviewed"];

function demoEntry(file: string, index: number, translated: boolean): TranslationEntry {
  const [speaker, original, translation] = LINES[index % LINES.length];
  const event = `EV${String(Math.floor(index / 2) + 1).padStart(3, "0")}`;
  return {
    id: `${file}:${event}:${index}`,
    fileContext: `${file} › ${event} ${speaker} › ${(index % 2) + 1}`,
    original,
    translation: translated ? translation : "",
    status: translated ? TRANSLATED_STATUSES[index % TRANSLATED_STATUSES.length] : "untranslated",
    notes: index % 7 === 0 ? "Revisar el tono." : "",
  };
}

export function createVisualDemoProject(): Project {
  const now = new Date().toISOString();

  return {
    id: "visual-demo",
    name: "Crónicas de Bruma",
    sourceLang: "ja",
    targetLang: "es",
    engine: "mz",
    gameDir: "D:/Demo/CronicasDeBruma",
    glossary: [
      { source: "霧の魔物", target: "monstruo de la niebla", caseSensitive: true },
      { source: "月の雫", target: "Gota de Luna", caseSensitive: true },
      { source: "回復薬", target: "Poción", caseSensitive: true },
      { source: "勇者", target: "héroe", caseSensitive: false },
      { source: "ゴールド", target: "oro", caseSensitive: false },
      { source: "宿屋", target: "posada", caseSensitive: false },
    ],
    styleGuide: [
      "Tutea a los personajes jóvenes; usa «usted» con nobles y ancianos.",
      "Textos de menú: 12 caracteres como máximo.",
      "Conserva los códigos de control de RPG Maker.",
    ],
    createdAt: now,
    updatedAt: now,
    files: FILES.map(([name, total, translated]) => ({
      relativePath: `data/${name}.json`,
      entries: Array.from({ length: total }, (_, index) => demoEntry(name, index, index < translated)),
    })),
  };
}

const DEMO_TEXT_FIT: TextFitReport = {
  wrapPlugin: null,
  calibration: [],
  wrappedEntries: 12,
  paginatedEntries: 3,
  pagesAdded: 4,
  overflowCount: 3,
  overflow: [
    { filePath: "data/Map002.json", entryId: "Map002:EV004:7", kind: "choice", width: 612, maxWidth: 560, lines: 1, maxLines: null },
    { filePath: "data/Items.json", entryId: "Items:EV002:3", kind: "description", width: 780, maxWidth: 784, lines: 3, maxLines: 2 },
    { filePath: "data/Actors.json", entryId: "Actors:EV001:1", kind: "name", width: 250, maxWidth: 180, lines: 1, maxLines: null },
  ],
};

/*
 * Backend mínimo para la demo en el navegador: responde el análisis de ajuste, la exportación y el
 * selector de carpeta con datos de muestra. El resto de comandos falla igual que sin Tauri.
 */
export function installVisualDemoBackend() {
  const target = window as unknown as { __TAURI_INTERNALS__?: { invoke: (cmd: string) => Promise<unknown> } };
  if (target.__TAURI_INTERNALS__) return;
  useCliUpdateStore.getState().setStatuses([
    { cli: "codex", state: "updated", fromVersion: "0.158.0", toVersion: "0.159.2", message: "Updated", skipReason: null },
  ]);
  target.__TAURI_INTERNALS__ = {
    invoke: async (cmd: string) => {
      await new Promise((resolve) => setTimeout(resolve, 150));
      if (cmd === "text_fit_analyze") return DEMO_TEXT_FIT;
      if (cmd === "game_inspect") {
        const packed = new URLSearchParams(window.location.search).get("inspect") === "packed";
        const inspection: GameInspection = packed
          ? { gameRoot: null, engine: "vxAce", packedArchive: "Game.rgss3a", blockReason: "packed_archive", dataFiles: 0, textFiles: null, entries: null }
          : { gameRoot: "D:/Juegos/CronicasDeBruma", engine: "mz", packedArchive: null, blockReason: null, dataFiles: 24, textFiles: 16, entries: 1022 };
        return inspection;
      }
      if (cmd === "plugin:dialog|open") return "D:/Exports/CronicasDeBruma";
      if (cmd === "project_location") return { path: "D:/Traducciones/cronicas-de-bruma.fairtl", saved: true };
      if (cmd === "export_project") {
        const result: ExportResult = {
          filesExported: 8,
          entriesApplied: 102,
          totalEntries: 160,
          warnings: ["Map003 › EV005: el número de líneas cambió (2 → 3)."],
          textFit: DEMO_TEXT_FIT,
        };
        return result;
      }
      throw new Error(`visual demo: ${cmd}`);
    },
  };
}

// Abre un panel desde la URL para capturas sin clics: ?visualDemo=1&panel=batch|context|styleGuide|settings|export|newProject
// (con panel=settings, &provider=codex abre ese proveedor).
export function openVisualDemoPanel(params: URLSearchParams) {
  const panel = params.get("panel");
  const store = usePanelStore.getState();
  if (panel === "context" || panel === "styleGuide") {
    store.openContext(panel === "styleGuide" ? "styleGuide" : "glossary");
  } else if (panel === "settings") {
    store.openSettings((params.get("provider") as ProviderType | null) ?? undefined);
  } else if (panel === "batch" || panel === "export" || panel === "newProject") {
    store.open(panel);
  }
}

// Pantalla de inicio con recientes: ?visualDemo=1&screen=welcome
export function seedVisualDemoRecents() {
  const day = 86_400_000;
  const now = Date.now();
  const { rememberProject } = useSettingsStore.getState();
  rememberProject({ path: "E:/Proyectos/senda-de-ceniza.fairtl", name: "Senda de Ceniza", engine: "vxAce", percent: 18, openedAt: new Date(now - 33 * day).toISOString() });
  rememberProject({ path: "D:/Traducciones/torre-del-eco.fairtl", name: "La Torre del Eco", engine: "xp", percent: 100, openedAt: new Date(now - day).toISOString() });
  rememberProject({ path: "D:/Traducciones/cronicas-de-bruma.fairtl", name: "Crónicas de Bruma", engine: "mz", percent: 64, openedAt: new Date(now).toISOString() });
}
