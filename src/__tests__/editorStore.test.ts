import { act } from "@testing-library/react";
import { useEditorStore } from "@/stores/editorStore";

describe("editorStore", () => {
  beforeEach(() => {
    useEditorStore.getState().reset();
  });

  it("tracks explicit match navigation separately from query resets", () => {
    act(() => {
      useEditorStore.getState().setMatchCount(3);
      useEditorStore.getState().nextMatch(3);
    });

    expect(useEditorStore.getState().currentMatchIndex).toBe(1);
    expect(useEditorStore.getState().matchNavigationVersion).toBe(1);

    act(() => {
      useEditorStore.getState().setFindQuery("hero");
    });

    expect(useEditorStore.getState().currentMatchIndex).toBe(0);
    expect(useEditorStore.getState().matchNavigationVersion).toBe(1);

    act(() => {
      useEditorStore.getState().prevMatch(3);
    });

    expect(useEditorStore.getState().currentMatchIndex).toBe(2);
    expect(useEditorStore.getState().matchNavigationVersion).toBe(2);

    // Replacing the last match shrinks the list: the current match must stay inside it.
    act(() => {
      useEditorStore.getState().setMatchCount(2);
    });
    expect(useEditorStore.getState().currentMatchIndex).toBe(1);

    act(() => {
      useEditorStore.getState().setMatchCount(0);
    });
    expect(useEditorStore.getState().currentMatchIndex).toBe(0);

    act(() => {
      useEditorStore.getState().setShowFindReplace(false);
    });

    expect(useEditorStore.getState().matchNavigationVersion).toBe(0);
  });
});
