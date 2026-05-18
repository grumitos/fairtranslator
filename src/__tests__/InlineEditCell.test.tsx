import { fireEvent, render, screen } from "@testing-library/react";
import { InlineEditCell } from "@/components/editor/translation-grid/InlineEditCell";
import type { EntryWithPath } from "@/components/editor/translation-grid/types";

describe("InlineEditCell", () => {
  it("opens the dialog with the latest inline value on F2", () => {
    const entry: EntryWithPath = {
      id: "Map001:text:1",
      _filePath: "Data/Map001.json",
      fileContext: "Map001 > Event 1",
      original: "Hello",
      translation: "",
      status: "untranslated",
      notes: "",
    };
    const onCommit = vi.fn();
    const onCancel = vi.fn();
    const onNavigate = vi.fn();
    const onOpenDialog = vi.fn();

    render(
      <InlineEditCell
        entry={entry}
        initialValue=""
        onCommit={onCommit}
        onCancel={onCancel}
        onNavigate={onNavigate}
        onOpenDialog={onOpenDialog}
        ariaLabel="Translation"
      />,
    );

    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "Hola" } });
    fireEvent.keyDown(textarea, { key: "F2" });

    expect(onCommit).toHaveBeenCalledWith(entry.id, entry._filePath, "Hola", "untranslated");
    expect(onOpenDialog).toHaveBeenCalledWith({
      ...entry,
      translation: "Hola",
      status: "draft",
    });
    expect(onCancel).not.toHaveBeenCalled();
    expect(onNavigate).not.toHaveBeenCalled();
  });
});
