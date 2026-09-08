import assert from "node:assert/strict";
import test from "node:test";
import { normalizeShortcut, matchesShortcut, formatShortcut } from "../src/lib/shortcuts/keys.ts";
import { emptyConfig, parseConfig, resolveBindings, validateBindings, localShortcutStorage, STORAGE_KEY } from "../src/lib/shortcuts/config.ts";
import { CommandRegistry } from "../src/lib/shortcuts/registry.ts";

const context = { workspaceId: "a", scopes: ["app", "workspace", "result"] };
function command(id, extra = {}) {
  return { id, title: id, scope: "workspace", shortcut: "Mod+Shift+Y", run() {}, ...extra };
}
function event(extra = {}) {
  return { key: "Y", code: "KeyY", ctrlKey: true, metaKey: false, altKey: false, shiftKey: true,
    repeat: false, isComposing: false, keyCode: 89, defaultPrevented: false,
    preventDefault() { this.defaultPrevented = true; }, ...extra };
}
function registry(commands, ctx = context, onError = assert.fail) {
  const instance = new CommandRegistry();
  instance.update(commands, ctx, onError);
  return instance;
}

test("portable modifiers, exact matching, shifted punctuation and invalid bindings", () => {
  assert.equal(normalizeShortcut(" shift + mod + y "), "Mod+Shift+Y");
  assert.equal(matchesShortcut("Mod+Shift+Y", event(), "other"), true);
  assert.equal(matchesShortcut("Mod+Shift+Y", event(), "mac"), false);
  assert.equal(matchesShortcut("Mod+Shift+Y", event({ ctrlKey: false, metaKey: true }), "mac"), true);
  assert.equal(matchesShortcut("Mod+Shift+Y", event({ altKey: true }), "other"), false);
  assert.equal(matchesShortcut("Mod+Shift+Slash", event({ key: "?", code: "Slash" }), "other"), true);
  assert.equal(formatShortcut("Mod+Slash", "other"), "Ctrl+/");
  for (const bad of ["Y", "Shift+Y", "Mod+", "Mod+Ctrl+Y", "Ctrl+Ctrl+Y", "Super+Y", "F12", "Ctrl+Shift+C"]) {
    assert.throws(() => normalizeShortcut(bad));
  }
});

test("versioned config validates untrusted values and preserves future command IDs", () => {
  const config = parseConfig({ version: 1, user: { "capture.ocr": "mod+shift+x" }, workspaces: {} });
  assert.equal(config.user["capture.ocr"], "Mod+Shift+X");
  for (const bad of [null, { version: 2 }, { version: 1, user: [], workspaces: {} },
    { version: 1, user: { x: 3 }, workspaces: {} }, { version: 1, user: {}, workspaces: [] }]) assert.throws(() => parseConfig(bad));
});

test("workspace overrides win, explicit null disables, absent values inherit", () => {
  const config = { ...emptyConfig(), user: { copy: "Mod+K" }, workspaces: { a: { copy: null } } };
  assert.equal(resolveBindings([command("copy")], config, "a")[0].shortcut, null);
  assert.equal(resolveBindings([command("copy")], config, "b")[0].shortcut, "Mod+K");
  assert.equal(resolveBindings([command("copy")], emptyConfig(), "b")[0].shortcut, "Mod+Shift+Y");
});

test("conflicts include platform aliases and every workspace, but allow nested scopes", () => {
  const commands = [command("one"), command("two", { shortcut: "Ctrl+Shift+Y" })];
  assert.throws(() => validateBindings(commands, emptyConfig(), "other"), /冲突/);
  validateBindings(commands, emptyConfig(), "mac");
  validateBindings([command("one"), command("two", { scope: "result" })], emptyConfig(), "other");
  assert.throws(() => validateBindings([command("one"), command("one", { shortcut: null })], emptyConfig(), "other"), /重复/);
  assert.throws(() => validateBindings([command("one"), command("two", { shortcut: "Mod+K" })],
    { ...emptyConfig(), workspaces: { b: { two: "Mod+Shift+Y" } } }, "other"), /工作区 b/);
});

test("IME, repeats, consumed events, AltGraph and editable input don't trigger commands", () => {
  let calls = 0;
  const instance = registry([command("copy", { run() { calls++; } })]);
  for (const extra of [{ isComposing: true }, { keyCode: 229 }, { repeat: true }, { defaultPrevented: true }, { getModifierState: () => true }]) {
    assert.equal(instance.handleKeydown(event(extra), false, "other"), false);
  }
  const typing = event();
  assert.equal(instance.handleKeydown(typing, true, "other"), false);
  assert.equal(typing.defaultPrevented, false);
  assert.equal(calls, 0);
});

test("explicit editable permission and scope precedence dispatch only one command", () => {
  const calls = [];
  const instance = registry([
    command("app", { scope: "app", run() { calls.push("app"); } }),
    command("result", { scope: "result", allowInEditable: true, run() { calls.push("result"); } }),
  ]);
  const key = event();
  assert.equal(instance.handleKeydown(key, true, "other"), true);
  assert.equal(key.defaultPrevented, true);
  assert.deepEqual(calls, ["result"]);
});

test("disabled inner command never falls through, dialogs and missing workspace isolate commands", async () => {
  const commands = [command("app", { scope: "app", run: assert.fail }), command("inner", { enabled: () => false })];
  assert.equal(registry(commands).handleKeydown(event(), false, "other"), false);
  assert.equal(registry(commands, { ...context, scopes: ["dialog"] }).handleKeydown(event(), false, "other"), false);
  assert.equal(await registry([command("copy")], { ...context, workspaceId: null }).execute("copy"), false);
});

test("pending command deduplicates within a workspace and pins its original context", async () => {
  let finish;
  const wait = new Promise((resolve) => { finish = resolve; });
  const runs = [];
  const cmd = command("capture", { async run(ctx) { await wait; runs.push(ctx.workspaceId); } });
  const instance = registry([cmd]);
  const first = instance.execute("capture");
  assert.equal(await instance.execute("capture"), false);
  instance.update([cmd], { ...context, workspaceId: "b" }, assert.fail);
  const second = instance.execute("capture");
  finish();
  assert.equal(await first, true);
  assert.equal(await second, true);
  assert.deepEqual(runs, ["a", "b"]);
});

test("app commands remain deduplicated across workspace changes", async () => {
  let finish;
  const wait = new Promise((resolve) => { finish = resolve; });
  const cmd = command("pin", { scope: "app", run: () => wait });
  const instance = registry([cmd]);
  const pending = instance.execute("pin");
  instance.update([cmd], { ...context, workspaceId: "b" }, assert.fail);
  assert.equal(await instance.execute("pin"), false);
  finish();
  await pending;
});

test("errors release pending commands and updated handlers don't capture stale state", async () => {
  const errors = [];
  const instance = registry([command("copy", { run() { throw new Error("clipboard denied"); } })], context, (error) => errors.push(error));
  assert.equal(await instance.execute("copy"), false);
  assert.match(errors[0], /clipboard denied/);
  let value = "";
  instance.update([command("copy", { run() { value = "new text"; } })], context, assert.fail);
  assert.equal(await instance.execute("copy"), true);
  assert.equal(value, "new text");
});

test("teardown aborts pending operations without reporting expected cancellation", async () => {
  let signal;
  const instance = registry([command("capture", { run(ctx) {
    signal = ctx.signal;
    return new Promise((_, reject) => ctx.signal.addEventListener("abort", () => reject(new Error("cancelled"))));
  } })]);
  const pending = instance.execute("capture", "native");
  instance.cancelAll();
  assert.equal(signal.aborted, true);
  assert.equal(await pending, false);
});

test("storage round trips only versioned bindings and exposes corruption or denied access", () => {
  const values = new Map();
  const previous = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  try {
    Object.defineProperty(globalThis, "localStorage", { configurable: true, value: {
      getItem: (key) => values.get(key) ?? null,
      setItem: (key, value) => values.set(key, value),
    } });
    assert.deepEqual(localShortcutStorage.load(), emptyConfig());
    const config = { ...emptyConfig(), user: { copy: null } };
    localShortcutStorage.save(config);
    assert.deepEqual(localShortcutStorage.load(), config);
    values.set(STORAGE_KEY, "bad json");
    assert.throws(() => localShortcutStorage.load());
    Object.defineProperty(globalThis, "localStorage", { configurable: true, get() { throw new Error("denied"); } });
    assert.throws(() => localShortcutStorage.save(config), /denied/);
  } finally {
    if (previous) Object.defineProperty(globalThis, "localStorage", previous);
    else delete globalThis.localStorage;
  }
});
