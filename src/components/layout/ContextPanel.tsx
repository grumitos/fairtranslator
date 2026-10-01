import { useTranslation } from "react-i18next";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { GlossaryEditor } from "@/components/layout/GlossaryPanel";
import { StyleGuideEditor } from "@/components/layout/StyleGuidePanel";
import type { ContextTab } from "@/components/layout/panelStore";
import { useProjectStore } from "@/stores/projectStore";

interface ContextPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  tab: ContextTab;
  onTabChange: (tab: ContextTab) => void;
}

// Contexto que acompaña a cada traducción: glosario y guía de estilo en un solo panel con pestañas.
export function ContextPanel({ open, onOpenChange, tab, onTabChange }: ContextPanelProps) {
  const { t } = useTranslation(["editor", "glossary"]);
  const project = useProjectStore((s) => s.project);
  const glossaryCount = project?.glossary.length ?? 0;
  const ruleCount = project?.styleGuide.length ?? 0;

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent>
        <SheetHeader>
          <SheetTitle>{t("editor:context.title")}</SheetTitle>
          <SheetDescription>
            {project ? `${project.name} · ${project.sourceLang.toUpperCase()} → ${project.targetLang.toUpperCase()}` : ""}
          </SheetDescription>
        </SheetHeader>
        <Tabs
          value={tab}
          onValueChange={(value) => onTabChange(value as ContextTab)}
          className="min-h-0 flex-1 gap-0"
        >
          <TabsList variant="line" className="mx-5 mt-3 w-[calc(100%-2.5rem)] justify-start">
            <TabsTrigger value="glossary">
              {t("glossary:title")}
              <span className="font-mono text-[11.5px] text-ink-4">{glossaryCount}</span>
            </TabsTrigger>
            <TabsTrigger value="styleGuide">
              {t("editor:toolbar.styleGuide")}
              <span className="font-mono text-[11.5px] text-ink-4">{ruleCount}</span>
            </TabsTrigger>
          </TabsList>
          <TabsContent value="glossary" className="flex min-h-0 flex-col">
            <GlossaryEditor />
          </TabsContent>
          <TabsContent value="styleGuide" className="flex min-h-0 flex-col">
            <StyleGuideEditor active={open && tab === "styleGuide"} />
          </TabsContent>
        </Tabs>
      </SheetContent>
    </Sheet>
  );
}
