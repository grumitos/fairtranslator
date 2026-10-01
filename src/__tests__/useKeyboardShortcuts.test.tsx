import { act, fireEvent, renderHook } from "@testing-library/react";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useEditorStore } from "@/stores/editorStore";

const saveMock = vi.fn();
const saveAsMock = vi.fn();

vi.mock("@/hooks/useProject", () => ({
  useProject: () => ({
    project: {
      id: "project-1",
      name: "Test Project",
    },
  }),
}));

// Guardar y "Guardar como" (y sus errores) viven en useProjectCommands; aquí solo se prueba el atajo.
vi.mock("@/components/layout/useProjectCommands", () => ({
  useProjectCommands: () => ({
    saveProject: (...args: unknown[]) => saveMock(...args),
    saveProjectAs: (...args: unknown[]) => saveAsMock(...args),
  }),
}));

describe("useKeyboardShortcuts", () => {
  beforeEach(() => {
    saveMock.mockReset();
    saveAsMock.mockReset();
    useEditorStore.getState().reset();
    document.body.innerHTML = "";
  });

  it("opens find/replace from a non-editable target", () => {
    renderHook(() => useKeyboardShortcuts());

    act(() => {
      fireEvent.keyDown(document.body, { key: "f", ctrlKey: true });
    });

    expect(useEditorStore.getState().showFindReplace).toBe(true);
  });

  it("keeps Ctrl+F for the field but saves with Ctrl+S outside dialogs", () => {
    saveMock.mockResolvedValue(undefined);
    renderHook(() => useKeyboardShortcuts());

    const input = document.createElement("input");
    document.body.appendChild(input);
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    const dialogInput = document.createElement("input");
    dialog.appendChild(dialogInput);
    document.body.appendChild(dialog);

    act(() => {
      input.focus();
      fireEvent.keyDown(input, { key: "f", ctrlKey: true });
    });
    expect(useEditorStore.getState().showFindReplace).toBe(false);

    let prevented = false;
    act(() => {
      prevented = !fireEvent.keyDown(dialogInput, { key: "s", ctrlKey: true });
    });
    expect(prevented).toBe(true);
    expect(saveMock).not.toHaveBeenCalled();

    act(() => {
      fireEvent.keyDown(input, { key: "s", ctrlKey: true });
    });
    expect(saveMock).toHaveBeenCalledOnce();
  });

  it("commits the inline editor before saving", () => {
    const order: string[] = [];
    saveMock.mockImplementation(async () => {
      order.push("save");
    });
    renderHook(() => useKeyboardShortcuts());

    const textarea = document.createElement("textarea");
    textarea.setAttribute("data-ft-inline-edit", "");
    textarea.addEventListener("blur", () => order.push("blur"));
    document.body.appendChild(textarea);

    act(() => {
      textarea.focus();
      fireEvent.keyDown(textarea, { key: "s", ctrlKey: true });
    });

    expect(order).toEqual(["blur", "save"]);
  });

  it("opens save as with Ctrl+Shift+S", () => {
    renderHook(() => useKeyboardShortcuts());

    act(() => {
      fireEvent.keyDown(document.body, { key: "S", ctrlKey: true, shiftKey: true });
    });

    expect(saveAsMock).toHaveBeenCalledOnce();
    expect(saveMock).not.toHaveBeenCalled();
  });
});
