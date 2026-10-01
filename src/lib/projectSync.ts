import * as ipc from "@/lib/ipc";
import { useProjectStore } from "@/stores/projectStore";
import type { Project } from "@/types/domain";

/**
 * Keeps the store (optimistic UI state) and the backend project (source of truth) in step.
 *
 * Entry edits are applied to the store first and sent to the backend afterwards. Flows that
 * read or persist the whole backend project must therefore wait for those requests, and must
 * not overwrite the store with a snapshot that is older than an edit typed in the meantime.
 */

/** Edit requests (project_update_entry / project_update_entries) the backend has not answered yet. */
const pendingEdits = new Set<Promise<unknown>>();

/** Registers an in-flight edit request so persist/refresh flows can wait for it. */
export function trackEdit<T>(request: Promise<T>): Promise<T> {
  const tracked = Promise.resolve(request);
  pendingEdits.add(tracked);
  const release = () => {
    pendingEdits.delete(tracked);
  };
  tracked.then(release, release);
  return tracked;
}

/** Resolves once every edit request issued so far has settled, successfully or not. */
export async function flushPendingEdits(): Promise<void> {
  while (pendingEdits.size > 0) {
    await Promise.allSettled([...pendingEdits]);
  }
}

const MAX_REFRESH_ATTEMPTS = 3;

/**
 * Replaces the store project with the backend copy without clobbering local edits:
 * pending edits are awaited first and the fetch is repeated if the user edited again
 * while it was in flight. After the last attempt the fetched project is adopted anyway.
 * A missing backend project (null) never replaces an open one.
 */
export async function refreshProjectFromBackend(
  markDirty: boolean | "preserve",
): Promise<Project | null> {
  let project: Project | null = null;
  for (let attempt = 1; attempt <= MAX_REFRESH_ATTEMPTS; attempt += 1) {
    await flushPendingEdits();
    const revision = useProjectStore.getState().editRevision;
    project = await ipc.projectGet();
    if (useProjectStore.getState().editRevision === revision) {
      break;
    }
  }
  if (project) {
    useProjectStore.getState().replaceProject(project, { markDirty });
  }
  return project;
}

/**
 * Runs a backend operation that writes the whole project to disk (save, style guide replace)
 * and then marks the store clean. Pending edits are flushed first so they reach the snapshot.
 * When the user edits while the operation is running, the project stays dirty instead of
 * claiming that the disk copy is up to date.
 *
 * The store is not refreshed from the backend afterwards: both already hold the same project and
 * fetching it again ships the whole project over IPC (tens of MiB on large games) on every save.
 */
export async function persistProject<T>(operation: () => Promise<T>): Promise<T> {
  await flushPendingEdits();
  const revision = useProjectStore.getState().editRevision;
  const result = await operation();
  const store = useProjectStore.getState();
  if (store.editRevision === revision) {
    store.markClean();
  }
  return result;
}
