import { Dialog } from "@base-ui/react/dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  Check,
  Copy,
  Expand,
  FileImage,
  LoaderCircle,
  ScanText,
  Trash2,
  X,
} from "lucide-react";
import { useLayoutEffect, useRef, useState, type RefObject } from "react";

import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { Bubble, BubbleContent } from "@/shared/ui/bubble";
import { Message, MessageAvatar, MessageContent, MessageFooter, MessageHeader } from "@/shared/ui/message";

import type { OcrResult } from "../ocr.types";
import { OcrRecordNavigation } from "./ocr-record-navigation";

const dateFormatter = new Intl.DateTimeFormat("zh-CN", {
  month: "numeric",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
});

type Props = {
  workspaceId: string | null;
  contentReady: boolean;
  workspaceName: string;
  documents: readonly OcrResult[];
  selectedImageId: string | null;
  copiedImageId: string | null;
  query: string;
  loading: boolean;
  dragging: boolean;
  dropZoneRef: RefObject<HTMLDivElement | null>;
  resultRef: RefObject<HTMLParagraphElement | null>;
  onSelect: (imageId: string) => void;
  onCopy: (document: OcrResult) => void;
  onDelete: (document: OcrResult) => void;
  onImageError: () => void;
};

export function OcrFeed({
  workspaceId,
  contentReady,
  workspaceName,
  documents,
  selectedImageId,
  copiedImageId,
  query,
  loading,
  dragging,
  dropZoneRef,
  resultRef,
  onSelect,
  onCopy,
  onDelete,
  onImageError,
}: Props) {
  const [expandedDocument, setExpandedDocument] = useState<OcrResult | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const boundaryRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const recordRefs = useRef(new Map<string, HTMLElement>());

  useLayoutEffect(() => {
    if (!workspaceId || !contentReady) return;
    const viewport = scrollRef.current;
    const content = contentRef.current;
    if (!viewport || !content) return;

    const scrollToBottom = () => {
      viewport.scrollTop = viewport.scrollHeight;
    };
    scrollToBottom();

    // Images and fonts can change the height after the initial positioning.
    const observer = new ResizeObserver(scrollToBottom);
    observer.observe(content);
    observer.observe(viewport);
    const stopFollowing = () => observer.disconnect();
    viewport.addEventListener("wheel", stopFollowing, { passive: true });
    viewport.addEventListener("touchstart", stopFollowing, { passive: true });
    window.addEventListener("pointerdown", stopFollowing);
    window.addEventListener("keydown", stopFollowing);

    return () => {
      observer.disconnect();
      viewport.removeEventListener("wheel", stopFollowing);
      viewport.removeEventListener("touchstart", stopFollowing);
      window.removeEventListener("pointerdown", stopFollowing);
      window.removeEventListener("keydown", stopFollowing);
    };
  }, [workspaceId, contentReady]);

  useLayoutEffect(() => {
    const viewport = scrollRef.current;
    const content = contentRef.current;
    const boundary = boundaryRef.current;
    if (!viewport || !content || !boundary) return;

    // Update only the visual boundary state, without rerendering records while scrolling.
    const updateBoundary = () => {
      const scrolled = viewport.scrollTop > 0 ? "true" : "false";
      if (boundary.dataset.scrolled !== scrolled) {
        boundary.dataset.scrolled = scrolled;
      }
    };
    updateBoundary();
    viewport.addEventListener("scroll", updateBoundary, { passive: true });
    const observer = new ResizeObserver(updateBoundary);
    observer.observe(content);
    observer.observe(viewport);

    return () => {
      observer.disconnect();
      viewport.removeEventListener("scroll", updateBoundary);
    };
  }, []);
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const visibleDocuments = normalizedQuery
    ? documents.filter(
        (document) =>
          document.fileName.toLocaleLowerCase().includes(normalizedQuery) ||
          document.text.toLocaleLowerCase().includes(normalizedQuery),
      )
    : documents;

  function jumpToRecord(imageId: string) {
    const viewport = scrollRef.current;
    const record = recordRefs.current.get(imageId);
    if (!viewport || !record) return;
    onSelect(imageId);
    const top = viewport.scrollTop + record.getBoundingClientRect().top - viewport.getBoundingClientRect().top - 24;
    viewport.scrollTo({ top, behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth" });
    record.focus({ preventScroll: true });
  }

  return (
    <section
      ref={dropZoneRef}
      className="relative flex min-w-0 flex-1 flex-col bg-background"
      aria-label={`${workspaceName}的识别内容`}
      aria-busy={loading}
    >
      {dragging ? (
        <div className="pointer-events-none absolute inset-3 z-20 grid place-items-center rounded-2xl border-2 border-dashed border-primary bg-background/95 shadow-xl">
          <div className="text-center">
            <FileImage className="mx-auto size-8 text-primary" aria-hidden="true" />
            <p className="mt-3 font-medium">松开图片，保存并识别</p>
            <p className="mt-1 text-xs text-muted-foreground">图片会存入当前工作空间</p>
          </div>
        </div>
      ) : null}

      <div ref={boundaryRef} className="ocr-feed-boundary flex min-h-0 flex-1">
        <div ref={scrollRef} className="min-h-0 min-w-0 flex-1 overflow-y-auto">
          <div ref={contentRef} className="mx-auto flex min-h-full w-full max-w-4xl flex-col px-4 py-4">
            {visibleDocuments.length > 0 ? (
              <div className="space-y-5">
                {visibleDocuments.map((document) => {
                  const selected = document.imageId === selectedImageId;
                  const copied = document.imageId === copiedImageId;
                  return (
                    <article
                      key={document.imageId}
                      ref={(node) => {
                        if (node) recordRefs.current.set(document.imageId, node);
                        else recordRefs.current.delete(document.imageId);
                      }}
                      tabIndex={-1}
                      aria-label={document.fileName}
                      className="space-y-2 rounded-md outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      onClick={() => onSelect(document.imageId)}
                    >
                      <Message align="end">
                        <MessageAvatar className="size-8 self-start bg-primary/15 text-primary-foreground" aria-label="图片消息">
                          <FileImage className="size-4" aria-hidden="true" />
                        </MessageAvatar>
                        <MessageContent className="w-fit max-w-44 items-end gap-1">
                          <button
                            type="button"
                            className={cn(
                              "group/image relative grid max-w-44 shrink-0 place-items-center overflow-hidden rounded-lg border bg-muted/30 p-1 outline-none hover:bg-muted/60 focus-visible:ring-2 focus-visible:ring-ring",
                              selected && "border-primary",
                            )}
                            onClick={(event) => {
                              event.stopPropagation();
                              onSelect(document.imageId);
                              setExpandedDocument(document);
                            }}
                            aria-label={`预览${document.fileName}`}
                            title="查看原图"
                          >
                            <img
                              src={convertFileSrc(document.imagePath)}
                              alt={document.fileName}
                              loading="lazy"
                              draggable={false}
                              className="max-h-28 max-w-40 object-contain"
                              onDragStart={(event) => event.preventDefault()}
                              onError={onImageError}
                            />
                            <span className="absolute right-1 bottom-1 grid size-5 place-items-center rounded bg-black/60 text-white opacity-0 transition-opacity group-hover/image:opacity-100 group-focus-visible/image:opacity-100">
                              <Expand className="size-3" aria-hidden="true" />
                            </span>
                          </button>
                        </MessageContent>
                      </Message>
                      <Message>
                        <MessageAvatar className="size-8 self-start bg-muted text-muted-foreground group-has-data-[slot=message-footer]/message:translate-y-0" aria-label="识别消息">
                          <ScanText className="size-4" aria-hidden="true" />
                        </MessageAvatar>
                        <MessageContent className="max-w-[85%] flex-1 items-start gap-1">
                          <MessageHeader className="px-0">识别结果</MessageHeader>
                          <Bubble variant={document.status === "failed" ? "destructive" : "outline"} className={cn("w-full max-w-full", selected && "rounded-xl ring-2 ring-primary/35")}>
                            <BubbleContent className="w-full">
                              <p
                                ref={selected ? resultRef : undefined}
                                tabIndex={selected ? -1 : undefined}
                                className={cn(
                                  "overflow-x-auto whitespace-pre font-mono text-sm leading-7 outline-none",
                                  !document.text && "text-muted-foreground",
                                  document.status === "failed" && "text-destructive",
                                )}
                              >
                                {document.text || document.errorMessage || "未识别到文字"}
                              </p>
                            </BubbleContent>
                          </Bubble>
                          <MessageFooter className="w-full flex-wrap gap-x-2 gap-y-1 px-0">
                            <span className="min-w-0 flex-1 truncate font-medium" title={document.fileName}>{document.fileName}</span>
                            <span className={cn("shrink-0 text-muted-foreground", document.status === "failed" && "text-destructive")}>
                              {document.status === "failed" ? "识别失败" : document.status === "unrecognized" ? "未识别" : document.status === "pending" ? "识别中" : "已识别"}
                            </span>
                            <time className="text-muted-foreground" dateTime={new Date(document.createdAt * 1000).toISOString()}>
                              {dateFormatter.format(document.createdAt * 1000)}
                            </time>
                            <div className="flex shrink-0 items-center gap-0.5">
                              <Button
                                size="icon-xs"
                                variant="ghost"
                                disabled={!document.text}
                                onClick={(event) => { event.stopPropagation(); onCopy(document); }}
                                aria-label={`复制${document.fileName}的识别文字`}
                                title="复制全文"
                              >
                                {copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
                              </Button>
                              <Button
                                size="icon-xs"
                                variant="ghost"
                                className="text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                                onClick={(event) => { event.stopPropagation(); onDelete(document); }}
                                aria-label={`删除${document.fileName}`}
                                title="删除图片和结果"
                              >
                                <Trash2 aria-hidden="true" />
                              </Button>
                            </div>
                          </MessageFooter>
                        </MessageContent>
                      </Message>
                    </article>
                  );
                })}
              </div>
            ) : (
              <div className="grid flex-1 place-items-center py-16 text-center">
                <div>
                  {normalizedQuery ? (
                    <>
                      <FileImage className="mx-auto size-9 text-muted-foreground/60" aria-hidden="true" />
                      <h2 className="mt-4 font-medium">没有匹配内容</h2>
                      <p className="mt-2 text-sm text-muted-foreground">换个关键词试试</p>
                    </>
                  ) : (
                    <>
                      <div className="mx-auto grid size-16 place-items-center rounded-2xl border border-dashed bg-zinc-50 text-primary dark:bg-zinc-900/60">
                        <ScanText className="size-7" strokeWidth={1.6} aria-hidden="true" />
                      </div>
                      <h2 className="mt-5 font-heading text-lg font-semibold">从第一张图片开始</h2>
                      <p className="mt-2 text-sm leading-6 text-muted-foreground">
                        拖入 PNG 或 JPG，或复制图片后按 Ctrl+V
                        <br />
                        图片与文字会自动保存到当前空间
                      </p>
                    </>
                  )}
                </div>
              </div>
            )}

            {loading ? (
              <div
                className="mt-3 flex items-center gap-2 px-2 py-2 text-sm"
                role="status"
              >
                <LoaderCircle className="size-4 text-primary motion-safe:animate-spin" aria-hidden="true" />
                正在保存并识别图片…
              </div>
            ) : null}
          </div>
        </div>


        {visibleDocuments.length > 5 ? (
          <OcrRecordNavigation documents={visibleDocuments} selectedImageId={selectedImageId} onJump={jumpToRecord} />
        ) : null}
      </div>

      <Dialog.Root
        open={Boolean(expandedDocument)}
        onOpenChange={(open) => {
          if (!open) setExpandedDocument(null);
        }}
      >
        <Dialog.Portal>
          <Dialog.Backdrop className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm" />
          <Dialog.Popup className="fixed inset-6 z-50 flex flex-col rounded-2xl border border-white/15 bg-zinc-950 p-3 text-white shadow-2xl outline-none">
            <div className="flex h-10 shrink-0 items-center justify-between gap-3 px-1">
              <Dialog.Title className="truncate text-sm font-medium">
                {expandedDocument?.fileName}
              </Dialog.Title>
              <Dialog.Close
                className="grid size-8 place-items-center rounded-lg text-zinc-300 outline-none hover:bg-white/10 hover:text-white focus-visible:ring-2 focus-visible:ring-white/60"
                aria-label="关闭图片预览"
              >
                <X className="size-4" aria-hidden="true" />
              </Dialog.Close>
            </div>
            <div className="flex min-h-0 flex-1 items-center justify-center overflow-auto p-3">
              {expandedDocument ? (
                <img
                  src={convertFileSrc(expandedDocument.imagePath)}
                  alt={expandedDocument.fileName}
                  className="max-h-full max-w-full object-contain"
                  onError={onImageError}
                />
              ) : null}
            </div>
          </Dialog.Popup>
        </Dialog.Portal>
      </Dialog.Root>
    </section>
  );
}