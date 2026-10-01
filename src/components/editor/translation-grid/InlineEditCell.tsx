import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import type { EntryStatus } from "@/types/domain";
import type { EntryWithPath } from "@/components/editor/translation-grid/types";
import { statusAfterEdit } from "@/lib/translationState";

interface InlineEditCellProps {
  entry: EntryWithPath;
  initialValue: string;
  wrapStyle?: CSSProperties;
  onCommit: (entryId: string, filePath: string, newValue: string, currentStatus: EntryStatus) => void;
  onCancel: () => void;
  onNavigate: (direction: "next" | "prev") => void;
  onOpenDialog: (entry: EntryWithPath) => void;
  ariaLabel: string;
}

export function InlineEditCell({
  entry,
  initialValue,
  wrapStyle,
  onCommit,
  onCancel,
  onNavigate,
  onOpenDialog,
  ariaLabel,
}: InlineEditCellProps) {
  const [value, setValue] = useState(initialValue);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const committedRef = useRef(false);
  const latestValueRef = useRef(value);
  latestValueRef.current = value;

  // Auto-focus and select on mount
  useEffect(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.focus();
      textarea.select();
    }
  }, []);

  // Auto-resize textarea height on value change
  useEffect(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = "auto";
      textarea.style.height = `${textarea.scrollHeight}px`;
    }
  }, [value]);

  // Save on unmount if value changed and not already committed
  useEffect(() => {
    return () => {
      if (latestValueRef.current !== initialValue && !committedRef.current) {
        committedRef.current = true;
        onCommit(entry.id, entry._filePath, latestValueRef.current, entry.status);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const commit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    if (value !== initialValue) {
      onCommit(entry.id, entry._filePath, value, entry.status);
    } else {
      onCancel();
    }
  }, [value, initialValue, entry.id, entry._filePath, entry.status, onCommit, onCancel]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Durante la composición de un IME (japonés, chino, coreano) Enter confirma la conversión, no la celda.
    if (e.nativeEvent.isComposing) return;
    if (e.key === "Tab") {
      e.preventDefault();
      if (!committedRef.current) {
        committedRef.current = true;
        if (value !== initialValue) {
          onCommit(entry.id, entry._filePath, value, entry.status);
        }
      }
      onNavigate(e.shiftKey ? "prev" : "next");
    } else if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (!committedRef.current) {
        committedRef.current = true;
        if (value !== initialValue) {
          onCommit(entry.id, entry._filePath, value, entry.status);
        }
      }
      onNavigate("next");
    } else if (e.key === "Escape") {
      e.preventDefault();
      committedRef.current = true;
      onCancel();
    } else if (e.key === "F2") {
      e.preventDefault();
      const nextStatus = statusAfterEdit(entry.status, value);
      if (!committedRef.current) {
        committedRef.current = true;
        if (value !== initialValue) {
          onCommit(entry.id, entry._filePath, value, entry.status);
        }
      }
      onOpenDialog({
        ...entry,
        translation: value,
        status: nextStatus,
      });
    }
  }, [value, initialValue, entry, onCommit, onCancel, onNavigate, onOpenDialog]);

  const handleBlur = useCallback(() => {
    commit();
  }, [commit]);

  return (
    <textarea
      ref={textareaRef}
      data-ft-inline-edit
      name={`translation-${entry.id}`}
      aria-label={ariaLabel}
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onKeyDown={handleKeyDown}
      onBlur={handleBlur}
      onDoubleClick={(e) => e.stopPropagation()}
      className="w-full resize-none rounded-md border border-line-chip bg-surface-1 px-2 py-1 text-sm leading-5 text-ink outline-none focus:border-ink-3"
      style={wrapStyle}
      rows={1}
    />
  );
}
