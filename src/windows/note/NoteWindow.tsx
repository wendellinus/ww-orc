import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getNote, updateNote } from "@/features/notes/api";
import { NoteAutosave } from "@/features/notes/model/autosave";
import { closeObject, setTopmost } from "@/shared/window/api";
import { WindowBar } from "@/shared/window/WindowBar";
const colors = [
  { id: "amber", label: "黄色" },
  { id: "mint", label: "绿色" },
  { id: "blue", label: "蓝色" },
  { id: "rose", label: "粉色" },
];
export default function NoteWindow({ id }: { id: string }) {
  const [text, setText] = useState("");
  const [color, setColor] = useState("amber");
  const [topmost, setPinned] = useState(false);
  const [status, setStatus] = useState("加载中…");
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const saver = useRef<NoteAutosave | null>(null);
  const closing = useRef(false);
  const draft = useRef({ text: "", color: "amber" });
  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const bind = (promise: Promise<() => void>) => {
      void promise
        .then((stop) => {
          if (disposed) stop();
          else stops.push(stop);
        })
        .catch((e) => {
          if (!disposed) setError(String(e));
        });
    };
    void Promise.all([getNote(id), getCurrentWindow().isAlwaysOnTop()])
      .then(([note, pinned]) => {
        if (disposed) return;
        draft.current = { text: note.text, color: note.color };
        setText(note.text);
        setColor(note.color);
        setPinned(pinned);
        saver.current = new NoteAutosave(
          note.revision,
          (d, r) => updateNote(id, d.text, d.color, r),
          (s) => {
            if (!disposed) setStatus(s);
          },
        );
        setStatus("已保存");
        setReady(true);
      })
      .catch((e) => {
        if (!disposed) setError(String(e));
      });
    bind(
      getCurrentWindow().onCloseRequested((event) => {
        event.preventDefault();
        void close();
      }),
    );
    bind(
      listen("desktop:flush-object", () => {
        void (async () => {
          try {
            if (!saver.current) throw new Error("便签尚未加载完成");
            await saver.current.flush();
            await invoke("window_ready_to_quit");
          } catch (e) {
            setError(String(e));
            await invoke("cancel_desktop_quit", {
              message: `便签保存失败：${String(e)}`,
            });
          }
        })();
      }),
    );
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
      saver.current?.dispose();
      saver.current = null;
    };
  }, [id]);
  async function close() {
    if (closing.current) return;
    closing.current = true;
    try {
      if (!saver.current) throw new Error("便签尚未加载完成");
      await saver.current.flush();
      await closeObject("note", id);
    } catch (e) {
      setError(String(e));
      closing.current = false;
    }
  }
  function change(next: { text: string; color: string }) {
    draft.current = next;
    setText(next.text);
    setColor(next.color);
    saver.current?.schedule(next);
  }
  async function toggle() {
    try {
      await setTopmost("note", id, !topmost);
      setPinned(!topmost);
    } catch (e) {
      setError(String(e));
    }
  }
  return (
    <main className="desktop-note" data-color={color}>
      <WindowBar
        onError={setError}
        title="便签"
        topmost={topmost}
        onToggle={() => void toggle()}
        onClose={() => void close()}
      />
      <div className="note-colors" aria-label="便签颜色">
        {colors.map((c) => (
          <button
            key={c.id}
            type="button"
            data-color={c.id}
            className="note-color"
            aria-label={c.label}
            aria-pressed={color === c.id}
            disabled={!ready}
            onClick={() => change({ ...draft.current, color: c.id })}
          />
        ))}
      </div>
      {error ? (
        <p className="desktop-error" role="alert">
          {error}
        </p>
      ) : null}
      <textarea
        className="note-editor"
        aria-label="便签内容"
        placeholder="写下要记住的事…"
        value={text}
        disabled={!ready}
        onChange={(e) => change({ ...draft.current, text: e.target.value })}
      />
      <footer className="note-status" role="status">
        {status}
      </footer>
    </main>
  );
}
