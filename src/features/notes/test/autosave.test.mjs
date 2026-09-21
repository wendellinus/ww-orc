import test from "node:test";
import assert from "node:assert/strict";
import { NoteAutosave } from "../model/autosave.ts";
test("close flush serializes in-flight changes using updated revisions", async () => {
  let release;
  const waiting = new Promise((r) => (release = r));
  const calls = [];
  const saver = new NoteAutosave(
    4,
    async (draft, revision) => {
      calls.push({ ...draft, revision });
      if (calls.length === 1) await waiting;
      return { revision: revision + 1 };
    },
    () => {},
  );
  saver.schedule({ text: "first", color: "amber" });
  const first = saver.flush();
  await Promise.resolve();
  saver.schedule({ text: "last", color: "mint" });
  const close = saver.flush();
  release();
  await Promise.all([first, close]);
  saver.dispose();
  assert.deepEqual(calls, [
    { text: "first", color: "amber", revision: 4 },
    { text: "last", color: "mint", revision: 5 },
  ]);
});
test("save failure retains latest draft for an explicit subsequent flush", async () => {
  let fail = true;
  const calls = [];
  const saver = new NoteAutosave(
    0,
    async (draft, revision) => {
      calls.push({ ...draft, revision });
      if (fail) throw Error("disk full");
      return { revision: 1 };
    },
    () => {},
  );
  saver.schedule({ text: "unsaved", color: "blue" });
  await assert.rejects(saver.flush(), /disk full/);
  fail = false;
  await saver.flush();
  saver.dispose();
  assert.equal(calls.length, 2);
  assert.equal(calls[1].text, "unsaved");
  assert.equal(calls[1].revision, 0);
});
