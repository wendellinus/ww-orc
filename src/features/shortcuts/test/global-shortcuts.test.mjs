import test from "node:test";
import assert from "node:assert/strict";
import { GlobalShortcuts, nativeShortcut } from "../model/global-shortcuts.ts";
import { resolveBindings, validateBindings, emptyConfig } from "../model/config.ts";

function setup() {
  const keys = new Map();
  const calls = [];
  const driver = {
    async register(key, handler) {
      if (key === "Control+K" || keys.has(key)) throw new Error("occupied");
      keys.set(key, handler);
    },
    async unregister(key) { keys.delete(key); },
  };
  return { keys, calls, native: new GlobalShortcuts(driver, id => calls.push(id)) };
}
test("global conflicts preserve working keys and never commit rejected settings", async () => {
  const { native, keys, calls } = setup();
  await native.replace([{ id: "show", shortcut: "Mod+Shift+O" }]);
  let committed = false;
  await assert.rejects(native.replace([{ id: "pin", shortcut: "Ctrl+P" }, { id: "show", shortcut: "Ctrl+K" }], () => committed = true));
  assert.equal(committed, false);
  assert.deepEqual([...keys.keys()], ["CommandOrControl+Shift+O"]);
  const handler = keys.values().next().value;
  handler({ state: "Released" });
  handler({ state: "Pressed" });
  assert.deepEqual(calls, ["show"]);
});
test("storage failures roll back registration; StrictMode cleanup and setup serialize", async () => {
  const { native, keys } = setup();
  const bindings = [{ id: "show", shortcut: "Ctrl+O" }];
  await Promise.all([native.replace(bindings), native.replace([]), native.replace(bindings)]);
  await assert.rejects(native.replace([{ id: "pin", shortcut: "Ctrl+P" }], () => { throw new Error("storage denied"); }));
  assert.deepEqual([...keys.keys()], ["Control+O"]);
  await native.replace([]);
  assert.equal(keys.size, 0);
});
test("global bindings ignore workspace overrides and conflict with local keys across scopes", () => {
  const commands = [{ id: "show", scope: "app", title: "show", global: true, shortcut: "Mod+O" }];
  const config = { ...emptyConfig(), workspaces: { a: { show: "Mod+K" } } };
  assert.equal(resolveBindings(commands, config, "a")[0].shortcut, "Mod+O");
  assert.throws(() => validateBindings([...commands, { id: "copy", scope: "workspace", title: "copy", shortcut: "Ctrl+O" }], config, "other"), /全局/);
  assert.equal(nativeShortcut("Meta+Slash"), "Super+Slash");
});
