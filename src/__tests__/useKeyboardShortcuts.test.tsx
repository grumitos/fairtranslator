import { act, fireEvent, renderHook, waitFor } from "@testing-library/react";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useEditorStore } from "@/stores/editorStore";

const saveMock = vi.fn();
const toastErrorMock = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock("sonner", () => ({
  toast: {
    error: (...args: unknown[]) => toastErrorMock(...args),
  },
}));

vi.mock("@/hooks/useProject", () => ({
  useProject: () => ({
    project: {
      id: "project-1",
      name: "Test Project",
    },
    save: (...args: unknown[]) => saveMock(...args),
  }),
}));

describe("useKeyboardShortcuts", () => {
  beforeEach(() => {
    saveMock.mockReset();
    toastErrorMock.mockReset();
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

  it("ignores shortcuts while typing in editable elements", () => {
    renderHook(() => useKeyboardShortcuts());

    const input = document.createElement("input");
    document.body.appendChild(input);

    act(() => {
      input.focus();
      fireEvent.keyDown(input, { key: "f", ctrlKey: true });
      fireEvent.keyDown(input, { key: "s", ctrlKey: true });
    });

    expect(useEditorStore.getState().showFindReplace).toBe(false);
    expect(saveMock).not.toHaveBeenCalled();
  });

  it("surfaces save errors triggered by Ctrl+S", async () => {
    saveMock.mockRejectedValueOnce(new Error("save failed"));
    renderHook(() => useKeyboardShortcuts());

    act(() => {
      fireEvent.keyDown(document.body, { key: "s", ctrlKey: true });
    });

    await waitFor(() => {
      expect(toastErrorMock).toHaveBeenCalledWith("error", {
        description: "save failed",
      });
    });
  });
});
