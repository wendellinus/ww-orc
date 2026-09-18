import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { X } from "lucide-react";

import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import {
  normalizeShortcut,
  formatShortcut,
  type Platform,
} from "../model/keys";
import { readRecordedShortcut } from "../model/recording";
import { type Command } from "../model/registry";
import { validateBindings, type ShortcutConfig } from "../model/config";

type Props = {
  commands: readonly Command[];
  config: ShortcutConfig;
  platform: Platform;
  onSave: (config: ShortcutConfig) => void | Promise<void>;
  onClose: () => void;
};

export function ShortcutSettings({
  commands,
  config,
  platform,
  onSave,
  onClose,
}: Props) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [draft, setDraft] = useState<Record<string, string>>(() =>
    Object.fromEntries(
      commands.map((command) => [
        command.id,
        Object.prototype.hasOwnProperty.call(config.user, command.id)
          ? (config.user[command.id] ?? "")
          : (command.shortcut ?? ""),
      ]),
    ),
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recordingId, setRecordingId] = useState<string | null>(null);
  const [recordingPreview, setRecordingPreview] = useState("");
  const [announcement, setAnnouncement] = useState("");

  useEffect(() => {
    const dialog = dialogRef.current!;
    const previousFocus = document.activeElement;
    dialog.showModal();
    return () => {
      dialog.close();
      if (previousFocus instanceof HTMLElement && previousFocus.isConnected)
        previousFocus.focus();
    };
  }, []);

  useEffect(() => {
    if (!recordingId) return;
    const cancelRecording = () => {
      setRecordingId(null);
      setRecordingPreview("");
      setAnnouncement("录制已取消，原快捷键未修改。");
    };
    const handleVisibility = () => {
      if (document.hidden) cancelRecording();
    };
    window.addEventListener("blur", cancelRecording);
    document.addEventListener("visibilitychange", handleVisibility);
    return () => {
      window.removeEventListener("blur", cancelRecording);
      document.removeEventListener("visibilitychange", handleVisibility);
    };
  }, [recordingId]);

  function stopRecording(message = "录制已取消，原快捷键未修改。") {
    setRecordingId(null);
    setRecordingPreview("");
    setAnnouncement(message);
    setError(null);
  }

  function startRecording(command: Command, target: HTMLElement) {
    setRecordingId(command.id);
    setRecordingPreview("");
    setError(null);
    setAnnouncement(
      "正在录制“" + command.title + "”：按下组合键，Esc 或 Tab 取消。",
    );
    target.focus();
  }

  function draftConfig(values = draft): ShortcutConfig {
    const user = { ...config.user };
    for (const command of commands) {
      const binding = values[command.id].trim()
        ? normalizeShortcut(values[command.id])
        : null;
      if (binding === (command.shortcut ?? null)) delete user[command.id];
      else user[command.id] = binding;
    }
    const next = { ...config, user };
    validateBindings(commands, next, platform);
    return next;
  }

  async function save() {
    if (recordingId || saving) return;
    setSaving(true);
    try {
      await onSave(draftConfig());
      onClose();
    } catch (cause) {
      setError(String(cause instanceof Error ? cause.message : cause));
    } finally {
      setSaving(false);
    }
  }

  function handleRecordingKey(event: KeyboardEvent<HTMLDialogElement>) {
    if (!recordingId) return;
    const key = event.nativeEvent;
    const bareKey =
      !key.ctrlKey && !key.metaKey && !key.altKey && !key.shiftKey;
    if (
      !key.isComposing &&
      key.keyCode !== 229 &&
      bareKey &&
      ["Escape", "Tab"].includes(key.key)
    ) {
      event.stopPropagation();
      if (key.key === "Escape") event.preventDefault();
      stopRecording();
      return;
    }
    // Capture before the app dispatcher, native button clicks and form submission.
    event.preventDefault();
    event.stopPropagation();
    try {
      const recorded = readRecordedShortcut(key, platform);
      if (recorded.kind === "ignore") return;
      if (recorded.kind === "modifiers") {
        setRecordingPreview(recorded.shortcut);
        return;
      }
      const next = { ...draft, [recordingId]: recorded.shortcut };
      draftConfig(next);
      setDraft(next);
      stopRecording(
        "已录制 " +
          formatShortcut(recorded.shortcut, platform) +
          "，点击保存后生效。",
      );
    } catch (cause) {
      setError(String(cause instanceof Error ? cause.message : cause));
    } finally {
      setSaving(false);
    }
  }

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="shortcut-title"
      aria-describedby="shortcut-description"
      className="m-auto max-h-[85dvh] w-[calc(100%-2rem)] max-w-xl overflow-auto rounded-xl border bg-background p-0 text-foreground shadow-xl backdrop:bg-black/40"
      onKeyDownCapture={handleRecordingKey}
      onKeyUpCapture={(event) => {
        if (!recordingId) return;
        event.preventDefault();
        event.stopPropagation();
        try {
          const recorded = readRecordedShortcut(event.nativeEvent, platform);
          if (recorded.kind === "modifiers")
            setRecordingPreview(recorded.shortcut);
        } catch {
          /* Invalid keys remain available for another recording attempt. */
        }
      }}
      onPointerDownCapture={(event) => {
        if (
          recordingId &&
          event.target instanceof Element &&
          event.target
            .closest("[data-shortcut-recorder]")
            ?.getAttribute("data-shortcut-recorder") !== recordingId
        ) {
          stopRecording();
        }
      }}
      onCancel={(event) => {
        event.preventDefault();
        if (recordingId) stopRecording();
        else onClose();
      }}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          save();
        }}
      >
        <header className="flex items-center justify-between border-b p-4">
          <h2 id="shortcut-title" className="font-semibold">
            快捷键设置
          </h2>
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            onClick={onClose}
            aria-label="关闭快捷键设置"
          >
            <X aria-hidden="true" />
          </Button>
        </header>
        <div className="space-y-4 p-4">
          <div className="space-y-3">
            {commands.map((command) => {
              const recording = recordingId === command.id;
              return (
                <div
                  key={command.id}
                  className="flex flex-wrap items-center justify-between gap-2"
                >
                  <label htmlFor={"shortcut-" + command.id} className="text-sm">
                    {command.title}{command.global ? "（全局）" : ""}
                    <span className="mt-1 block text-xs text-muted-foreground">
                      默认：{formatShortcut(command.shortcut ?? null, platform)}
                    </span>
                  </label>
                  <div className="relative w-full min-w-0 sm:w-48">
                    <input
                      type="text"
                      readOnly
                      id={"shortcut-" + command.id}
                      data-shortcut-recorder={command.id}
                      aria-label={command.title + "快捷键"}
                      aria-describedby={
                        recording ? "shortcut-recording-status" : undefined
                      }
                      aria-invalid={recording && Boolean(error)}
                      title={recording ? "点击取消录制" : "点击录制快捷键"}
                      value={
                        recording
                          ? recordingPreview
                            ? formatShortcut(recordingPreview, platform) +
                              " + …"
                            : "请按下组合键…"
                          : formatShortcut(draft[command.id] || null, platform)
                      }
                      onClick={(event) => {
                        if (recording) stopRecording();
                        else startRecording(command, event.currentTarget);
                      }}
                      onKeyDown={(event) => {
                        if (!recording && ["Enter", " "].includes(event.key)) {
                          event.preventDefault();
                          event.stopPropagation();
                          startRecording(command, event.currentTarget);
                        }
                      }}
                      className={cn(
                        "h-9 w-full min-w-0 cursor-pointer truncate rounded-md border bg-transparent py-2 pr-9 pl-3 text-sm outline-none hover:bg-muted/40 focus-visible:ring-2 focus-visible:ring-ring",
                        recording &&
                          "border-primary bg-primary/5 ring-2 ring-primary/25",
                      )}
                    />
                    {draft[command.id] || recording ? (
                      <button
                        type="button"
                        data-shortcut-recorder={command.id}
                        aria-label={"清除" + command.title + "快捷键"}
                        title="清除快捷键"
                        onPointerDown={(event) => event.preventDefault()}
                        onClick={() => {
                          stopRecording(
                            "已清除“" +
                              command.title +
                              "”快捷键，点击保存后禁用。",
                          );
                          setDraft((previous) => ({
                            ...previous,
                            [command.id]: "",
                          }));
                        }}
                        className="absolute top-1/2 right-1 grid size-6 -translate-y-1/2 place-items-center rounded text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
                      >
                        <X className="size-3.5" aria-hidden="true" />
                      </button>
                    ) : null}
                  </div>
                </div>
              );
            })}
          </div>
          <p
            id="shortcut-recording-status"
            role="status"
            className="min-h-5 text-xs leading-5 text-muted-foreground"
          >
            {announcement}
          </p>
          {error ? (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          ) : null}
        </div>
        <footer className="flex flex-wrap items-center justify-between gap-3 border-t p-4">
          <Button
            type="button"
            variant="ghost"
            onClick={() => {
              stopRecording("已恢复默认，点击保存后生效。");
              setDraft(
                Object.fromEntries(
                  commands.map((command) => [
                    command.id,
                    command.shortcut ?? "",
                  ]),
                ),
              );
            }}
          >
            恢复默认
          </Button>
          <div className="flex gap-2">
            <Button type="button" variant="outline" onClick={onClose} disabled={saving}>
              取消
            </Button>
            <Button type="submit" disabled={Boolean(recordingId) || saving}>
              保存
            </Button>
          </div>
        </footer>
      </form>
    </dialog>
  );
}
