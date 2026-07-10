import { render, screen } from "@testing-library/react";
import { IconAction } from "@/components/layout/toolbar/IconAction";
import { TooltipProvider } from "@/components/ui/tooltip";

describe("IconAction responsive target", () => {
  it("uses a 40px mobile target, returns to 28px at md and keeps the icon size", () => {
    render(
      <TooltipProvider>
        <IconAction label="Export" onClick={() => undefined}>
          <svg className="size-4" aria-hidden="true" />
        </IconAction>
      </TooltipProvider>,
    );

    const action = screen.getByRole("button", { name: "Export" });
    expect(action).toHaveClass("size-10", "md:size-7");
    expect(action.querySelector("svg")).toHaveClass("size-4");
  });
});
