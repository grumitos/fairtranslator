import { describe, expect, it } from "vitest";
import { toErrorMessage } from "@/lib/ipc";

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
