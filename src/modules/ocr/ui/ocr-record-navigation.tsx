import { useLayoutEffect, useRef } from "react";

import { cn } from "@/shared/lib/utils";
import { HoverCard, HoverCardContent, HoverCardTrigger } from "@/shared/ui/hover-card";
import type { OcrResult } from "../ocr.types";

function previewParagraphs(text: string) {
  const paragraphs = text.split(/\n\s*\n/).map((paragraph) => paragraph.trim()).filter(Boolean);
  const sections = paragraphs.length > 1 ? paragraphs : text.split(/\n/).map((line) => line.trim()).filter(Boolean);
  return sections.slice(0, 3).map((section) => {
    const characters = Array.from(section);
    return characters.slice(0, 400).join("") + (characters.length > 400 ? "…" : "");
  });
}

export function OcrRecordNavigation({ documents, selectedImageId, onJump }: {
  documents: readonly OcrResult[];
  selectedImageId: string | null;
  onJump: (imageId: string) => void;
}) {
  const navRef = useRef<HTMLElement>(null);
  const selectedRef = useRef<HTMLElement>(null);
  useLayoutEffect(() => {
    const nav = navRef.current;
    const item = selectedRef.current;
    if (!nav || !item) return;
    const navBounds = nav.getBoundingClientRect();
    const itemBounds = item.getBoundingClientRect();
    if (itemBounds.top < navBounds.top || itemBounds.bottom > navBounds.bottom) {
      nav.scrollTop += itemBounds.top - navBounds.top - (nav.clientHeight - item.clientHeight) / 2;
    }
  }, [selectedImageId, documents]);

  return (
    <aside className="absolute top-1/2 right-2 z-10 flex max-h-[70%] w-20 -translate-y-1/2 flex-col" aria-label="记录文字导航">
      <nav ref={navRef} className="min-h-0 space-y-0.5 overflow-y-auto overscroll-contain rounded-lg border bg-background/85 p-1 shadow-md backdrop-blur-md" aria-label="跳转到识别记录">
        {documents.map((document, index) => {
          const selected = document.imageId === selectedImageId;
          const text = document.text.trim() || document.errorMessage || "未识别到文字";
          const characters = Array.from(text.replace(/\s+/g, " "));
          const summary = characters.slice(0, 5).join("") + (characters.length > 5 ? "…" : "");
          const paragraphs = previewParagraphs(text);
          return (
            <HoverCard key={document.imageId}>
              <HoverCardTrigger
                ref={selected ? (node) => { selectedRef.current = node; } : undefined}
                render={<button type="button" />}
                delay={200}
                closeDelay={150}
                aria-current={selected ? "location" : undefined}
                aria-label={`跳转到第 ${index + 1} 条记录：${summary}`}
                onClick={() => onJump(document.imageId)}
                className={cn("block w-full truncate rounded-md border border-transparent px-1.5 py-1 text-left text-[10px] text-muted-foreground outline-none transition-colors hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50", selected && "border-primary/30 bg-primary/10 text-foreground", document.status === "failed" && "text-destructive")}
              >
                {summary}
              </HoverCardTrigger>
              <HoverCardContent side="left" align="center" sideOffset={12} className="w-80 max-w-[calc(100vw-2rem)]">
                <div className="max-h-72 space-y-3 overflow-y-auto overscroll-contain text-sm leading-6">
                  {paragraphs.map((paragraph, paragraphIndex) => <p key={paragraphIndex} className="whitespace-pre-wrap wrap-break-word">{paragraph}</p>)}
                </div>
              </HoverCardContent>
            </HoverCard>
          );
        })}
      </nav>
    </aside>
  );
}
