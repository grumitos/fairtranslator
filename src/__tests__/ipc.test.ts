import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { apikeySave, projectCreate, projectGet, toErrorMessage, translateBatchCancel } from "@/lib/ipc";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("toErrorMessage", () => {
  it("returns message from Error instances", () => {
    expect(toErrorMessage(new Error("boom"))).toBe("boom");
  });

  it("returns plain string errors", () => {
    expect(toErrorMessage("plain failure")).toBe("plain failure");
  });

  it("extracts message from tauri-style payload", () => {
    const err = { kind: "NotFound", message: "No RPG Maker game root found" };
    expect(toErrorMessage(err)).toBe("No RPG Maker game root found");
  });

  it("extracts nested error message", () => {
    const err = { error: { message: "Nested failure" } };
    expect(toErrorMessage(err)).toBe("Nested failure");
  });
});

describe("ipc command payloads", () => {
  it("invokes project_create with the documented payload shape", async () => {
    invokeMock.mockResolvedValue({
      id: "project-1",
      name: "Demo",
      sourceLang: "ja",
      targetLang: "es",
      engine: "mv",
      gameDir: "C:/Games/Demo",
      files: [],
      glossary: [],
      styleGuide: [],
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    });

    await projectCreate("C:/Games/Demo", "Demo", "ja", "es");

    expect(invokeMock).toHaveBeenCalledWith("project_create", {
      gameDir: "C:/Games/Demo",
      name: "Demo",
      sourceLang: "ja",
      targetLang: "es",
    });
  });

  it("invokes no-payload commands without an empty object", async () => {
    invokeMock.mockResolvedValue(null);

    await projectGet();

    expect(invokeMock).toHaveBeenCalledWith("project_get", undefined);
  });

  it("normalizes optional API key settings to null for IPC", async () => {
    invokeMock.mockResolvedValue(undefined);

    await apikeySave("codex", "", undefined, undefined, undefined, undefined, "codex", undefined);

    expect(invokeMock).toHaveBeenCalledWith("apikey_save", {
      provider: "codex",
      apiKey: "",
      model: null,
      endpoint: null,
      organization: null,
      project: null,
      commandPath: "codex",
      profile: null,
      authMode: null,
    });
  });

  it("targets batch cancellation by run ID", async () => {
    invokeMock.mockResolvedValue(true);

    await translateBatchCancel("42");

    expect(invokeMock).toHaveBeenCalledWith("translate_batch_cancel", { runId: "42" });
  });
});
