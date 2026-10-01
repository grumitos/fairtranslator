import type {
  BatchProgress,
  EntryStatus,
  GameInspection,
  GlossaryTerm,
  Project,
  TranslationEntry,
} from "@/types/domain";

/**
 * In-memory stand-in for the Tauri/Rust backend used by flow tests.
 *
 * It mirrors the semantics the frontend relies on (see src-tauri/src/commands):
 * - commands run one at a time behind a lock, in arrival order;
 * - entry updates live only in memory until `project_save`;
 * - a blank translation always forces the `untranslated` status;
 * - `style_guide_replace` persists only the guide (project.json) of a project already on disk;
 * - batch runs report through `translation:progress` / `translation:complete`, and the
 *   terminal event may be emitted before `translate_batch_start` returns.
 *
 * Tests can hold a command (a slow disk write that keeps the lock), make the next call of a
 * command fail, and inspect what reached "disk". Unknown commands throw so contract drift shows up.
 */
type Args = Record<string, unknown> | undefined;
type Listener = (event: { payload: unknown }) => void;
type Handler = (args: Args) => unknown;
type EntryUpdate = {
  filePath: string;
  entryId: string;
  translation: string;
  status: EntryStatus;
  notes?: string | null;
};

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;

export class FakeBackend {
  project: Project | null = null;
  /** Last snapshot persisted through project_save / style_guide_replace. */
  disk: Project | null = null;
  savePath: string | null = null;
  calls: Array<{ command: string; args: Args }> = [];
  /** Project handed out by project_create. */
  gameFixture: Project | null = null;
  /** Reply of game_inspect; by default it describes gameFixture (or no game). */
  inspection: GameInspection | null = null;

  private lockTail: Promise<void> = Promise.resolve();
  private holds = new Map<string, Promise<void>>();
  private failures = new Map<string, string[]>();
  private listeners = new Map<string, Set<Listener>>();
  private activeRun: { runId: string; progress: BatchProgress } | null = null;
  private runCounter = 0;
  private clock = 0;

  reset() {
    this.project = null;
    this.disk = null;
    this.savePath = null;
    this.calls = [];
    this.gameFixture = null;
    this.inspection = null;
    this.lockTail = Promise.resolve();
    this.holds.clear();
    this.failures.clear();
    this.listeners.clear();
    this.activeRun = null;
    this.runCounter = 0;
    this.clock = 0;
  }

  /** Delays the reply of every call of `command` until the returned function runs; the lock stays taken meanwhile. */
  hold(command: string): () => void {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    this.holds.set(command, gate);
    return () => {
      this.holds.delete(command);
      release();
    };
  }

  /** Makes the next call of `command` reject with a backend-style message. */
  failNext(command: string, message: string) {
    const queue = this.failures.get(command) ?? [];
    queue.push(message);
    this.failures.set(command, queue);
  }

  callsTo(command: string) {
    return this.calls.filter((call) => call.command === command);
  }

  // ── Events ──────────────────────────────────────────────────────────

  listen = async (event: string, handler: Listener) => {
    const set = this.listeners.get(event) ?? new Set<Listener>();
    set.add(handler);
    this.listeners.set(event, set);
    return () => {
      set.delete(handler);
    };
  };

  listenerCount(event: string) {
    return this.listeners.get(event)?.size ?? 0;
  }

  emit(event: string, payload: unknown) {
    for (const handler of [...(this.listeners.get(event) ?? [])]) {
      handler({ payload });
    }
  }

  // ── Batch simulation ────────────────────────────────────────────────

  /** Reports partial progress of the active run. */
  reportProgress(patch: Partial<BatchProgress>) {
    if (!this.activeRun) throw new Error("no active batch run");
    this.activeRun.progress = { ...this.activeRun.progress, ...patch };
    this.emit("translation:progress", this.activeRun.progress);
  }

  /**
   * Finishes the active run: writes `translations` into the in-memory project as drafts
   * (what the orchestrator does chunk by chunk) and emits the terminal event.
   */
  finishRun(options: {
    translations?: Record<string, string>;
    cancelled?: boolean;
    failedEntries?: number;
  } = {}) {
    const run = this.activeRun;
    const project = this.project;
    if (!run || !project) throw new Error("no active batch run");
    let translated = 0;
    for (const file of project.files) {
      for (const entry of file.entries) {
        const text = options.translations?.[entry.id];
        if (text !== undefined) {
          entry.translation = text;
          entry.status = "draft";
          translated += 1;
        }
      }
    }
    this.touch();
    const terminal: BatchProgress = {
      ...run.progress,
      cancelled: options.cancelled ?? false,
      filesProcessed: run.progress.totalFiles,
      chunksProcessed: run.progress.totalChunks,
      entriesTranslated: translated,
      failedEntries: options.failedEntries ?? 0,
      currentFile: "",
    };
    this.activeRun = null;
    this.emit("translation:complete", terminal);
    return terminal;
  }

  // ── IPC entry point ─────────────────────────────────────────────────

  invoke = async (command: string, args?: Args): Promise<unknown> => {
    this.calls.push({ command, args: args ? clone(args) : args });

    const previous = this.lockTail;
    let unlock!: () => void;
    this.lockTail = new Promise<void>((resolve) => {
      unlock = resolve;
    });
    await previous;

    try {
      const queue = this.failures.get(command);
      if (queue && queue.length > 0) {
        // Tauri rejects with the serialized AppError: { kind, message }.
        throw { kind: "Project", message: queue.shift() };
      }

      const handler = this.commands[command];
      if (!handler) throw new Error(`FakeBackend: unmocked command ${command}`);
      const result = handler(args);

      const hold = this.holds.get(command);
      if (hold) await hold;
      return result;
    } finally {
      unlock();
    }
  };

  // ── Helpers ─────────────────────────────────────────────────────────

  private touch() {
    if (this.project) {
      this.clock += 1;
      this.project.updatedAt = `2026-01-01T00:00:${String(this.clock).padStart(2, "0")}.000Z`;
    }
  }

  private requireProject(): Project {
    if (!this.project) throw { kind: "Project", message: "No hay ningún proyecto abierto" };
    return this.project;
  }

  private findEntry(filePath: string, entryId: string): TranslationEntry {
    const file = this.requireProject().files.find((candidate) => candidate.relativePath === filePath);
    if (!file) throw { kind: "NotFound", message: `Archivo no encontrado: ${filePath}` };
    const entry = file.entries.find((candidate) => candidate.id === entryId);
    if (!entry) throw { kind: "NotFound", message: `Entrada no encontrada: ${entryId}` };
    return entry;
  }

  private applyUpdate(update: EntryUpdate) {
    const entry = this.findEntry(update.filePath, update.entryId);
    entry.translation = update.translation;
    entry.status = update.translation.trim() === "" ? "untranslated" : update.status;
    if (update.notes !== undefined && update.notes !== null) entry.notes = update.notes;
  }

  private pendingEntries(filePaths: string[]) {
    const project = this.requireProject();
    const files = project.files.filter(
      (file) => filePaths.length === 0 || filePaths.includes(file.relativePath),
    );
    const pending = files.flatMap((file) =>
      file.entries.filter(
        (entry) =>
          entry.original.trim() !== "" &&
          (entry.status === "untranslated" || entry.translation.trim() === ""),
      ),
    );
    return { files, pending };
  }

  // ── Commands (names mirror src-tauri/src/lib.rs) ────────────────────

  private commands: Record<string, Handler> = {
    game_inspect: () => {
      if (this.inspection) return clone(this.inspection);
      const fixture = this.gameFixture;
      return {
        gameRoot: fixture?.gameDir ?? null,
        engine: fixture?.engine ?? null,
        packedArchive: null,
        blockReason: fixture ? null : "no_game",
        dataFiles: fixture?.files.length ?? 0,
        textFiles: fixture ? fixture.files.length : null,
        entries: fixture ? fixture.files.reduce((sum, file) => sum + file.entries.length, 0) : null,
      } satisfies GameInspection;
    },

    project_create: (args) => {
      if (args?.sourceLang === args?.targetLang) {
        throw { kind: "Project", message: "El idioma de origen y destino deben ser distintos" };
      }
      if (!this.gameFixture) {
        throw { kind: "NotFound", message: "No se encontró la carpeta del juego" };
      }
      const project = clone(this.gameFixture);
      project.name = String(args?.name ?? project.name);
      project.sourceLang = String(args?.sourceLang);
      project.targetLang = String(args?.targetLang);
      this.project = project;
      this.savePath = `${String(args?.gameDir)}.fairtl`;
      return clone(project);
    },

    project_open: (args) => {
      if (!this.disk) {
        throw { kind: "NotFound", message: `No se encontró el proyecto: ${String(args?.path)}` };
      }
      this.project = clone(this.disk);
      this.savePath = String(args?.path);
      return clone(this.project);
    },

    project_get: () => (this.project ? clone(this.project) : null),

    project_location: () =>
      this.project && this.savePath ? { path: this.savePath, saved: this.disk !== null } : null,

    project_save: (args) => {
      const project = this.requireProject();
      const target = (args?.path as string | null | undefined) ?? this.savePath;
      if (!target) throw { kind: "Project", message: "No hay ruta de guardado. Usa Guardar como." };
      this.touch();
      this.disk = clone(project);
      this.savePath = target;
      return null;
    },

    project_update_entry: (args) => {
      this.applyUpdate(args as unknown as EntryUpdate);
      this.touch();
      return null;
    },

    project_update_entries: (args) => {
      const updates = (args?.updates ?? []) as EntryUpdate[];
      if (updates.length === 0) return null;
      const snapshot = clone(this.requireProject());
      try {
        for (const update of updates) this.applyUpdate(update);
      } catch (error) {
        this.project = snapshot; // atomic: nothing is applied when one update fails
        throw error;
      }
      this.touch();
      return null;
    },

    glossary_add: (args) => {
      const project = this.requireProject();
      const source = String(args?.source);
      if (project.glossary.some((term) => term.source === source)) {
        throw { kind: "Project", message: `El término ya existe en el glosario: "${source}"` };
      }
      project.glossary.push({
        source,
        target: String(args?.target),
        caseSensitive: Boolean(args?.caseSensitive),
      });
      this.touch();
      return clone(project.glossary);
    },

    glossary_update: (args) => {
      const project = this.requireProject();
      const term = project.glossary.find((candidate) => candidate.source === args?.source);
      if (!term) {
        throw { kind: "NotFound", message: `Término no encontrado: "${String(args?.source)}"` };
      }
      term.target = String(args?.target);
      term.caseSensitive = Boolean(args?.caseSensitive);
      this.touch();
      return clone(project.glossary);
    },

    glossary_remove: (args) => {
      const project = this.requireProject();
      const before = project.glossary.length;
      project.glossary = project.glossary.filter((term: GlossaryTerm) => term.source !== args?.source);
      if (project.glossary.length === before) {
        throw { kind: "NotFound", message: `Término no encontrado: "${String(args?.source)}"` };
      }
      this.touch();
      return clone(project.glossary);
    },

    glossary_list: () => clone(this.requireProject().glossary),

    style_guide_list: () => clone(this.requireProject().styleGuide),

    style_guide_replace: (args) => {
      const project = this.requireProject();
      const seen = new Set<string>();
      project.styleGuide = ((args?.items ?? []) as string[])
        .map((item) => item.trim())
        .filter((item) => {
          if (item === "" || seen.has(item)) return false;
          seen.add(item);
          return true;
        });
      this.touch();
      // Only project.json is rewritten: unsaved entry edits stay out of the disk copy.
      if (this.disk) this.disk = { ...clone(this.disk), styleGuide: clone(project.styleGuide) };
      return clone(project.styleGuide);
    },

    translate_batch_start: (args) => {
      if (this.activeRun) {
        throw { kind: "Project", message: "Ya hay una traducción batch en curso" };
      }
      const { files, pending } = this.pendingEntries((args?.filePaths as string[] | undefined) ?? []);
      this.runCounter += 1;
      const initial: BatchProgress = {
        runId: String(this.runCounter),
        cancelled: false,
        filesProcessed: 0,
        totalFiles: pending.length > 0 ? files.length : 0,
        chunksProcessed: 0,
        totalChunks: pending.length > 0 ? 1 : 0,
        entriesTranslated: 0,
        totalEntries: pending.length,
        currentFile: "",
        currentFileChunksProcessed: 0,
        currentFileTotalChunks: 0,
        failedEntries: 0,
        failedBatches: 0,
        warnings: [],
      };
      if (pending.length === 0) {
        // Nothing to translate: the real backend emits the terminal event BEFORE returning.
        this.emit("translation:complete", { ...initial });
        return initial;
      }
      this.activeRun = { runId: initial.runId, progress: { ...initial } };
      return initial;
    },

    translate_batch_cancel: (args) => this.activeRun?.runId === String(args?.runId),

    export_project: () => {
      const project = this.requireProject();
      let applied = 0;
      let total = 0;
      let files = 0;
      for (const file of project.files) {
        let fileApplied = 0;
        for (const entry of file.entries) {
          total += 1;
          if (entry.status !== "untranslated" && entry.translation.trim() !== "") fileApplied += 1;
        }
        applied += fileApplied;
        if (fileApplied > 0) files += 1;
      }
      return { filesExported: files, entriesApplied: applied, totalEntries: total, warnings: [] as string[], textFit: emptyTextFit() };
    },

    text_fit_analyze: () => {
      this.requireProject();
      return emptyTextFit();
    },
  };
}

function emptyTextFit() {
  return {
    wrapPlugin: null,
    calibration: [] as string[],
    wrappedEntries: 0,
    paginatedEntries: 0,
    pagesAdded: 0,
    overflowCount: 0,
    overflow: [],
  };
}

export const backend = new FakeBackend();

/** Module factories for vi.mock("@tauri-apps/api/core") and vi.mock("@tauri-apps/api/event"). */
export const tauriCoreMock = {
  invoke: (command: string, args?: Args) => backend.invoke(command, args),
  isTauri: () => true,
};
export const tauriEventMock = {
  listen: (event: string, handler: Listener) => backend.listen(event, handler),
};

export function makeProject(overrides: Partial<Project> = {}): Project {
  const entries = (prefix: string, count: number): TranslationEntry[] =>
    Array.from({ length: count }, (_, index) => ({
      id: `${prefix}:name:${index + 1}`,
      fileContext: `${prefix} > #${index + 1} > name`,
      original: `Original ${prefix} ${index + 1}`,
      translation: "",
      status: "untranslated" as const,
      notes: "",
    }));

  return {
    id: "project-1",
    name: "Fake Game",
    sourceLang: "ja",
    targetLang: "es",
    engine: "mv",
    gameDir: "C:/Games/Fake",
    files: [
      { relativePath: "data/Actors.json", entries: entries("Actors", 3) },
      { relativePath: "data/Items.json", entries: entries("Items", 2) },
    ],
    glossary: [],
    styleGuide: [],
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    ...overrides,
  };
}
