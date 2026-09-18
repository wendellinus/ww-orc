import assert from "node:assert/strict";
import test from "node:test";
import { readRecordedShortcut } from "../model/recording.ts";
import { matchesShortcut } from "../model/keys.ts";

const input = (extra = {}) => ({key:"k",code:"KeyK",ctrlKey:false,metaKey:false,altKey:false,shiftKey:false,...extra});

test("recording maps platform primary modifiers and matches the original keyboard event", () => {
  for (const [platform, event, expected] of [
    ["other", input({ctrlKey:true,altKey:true}), "Mod+Alt+K"],
    ["mac", input({metaKey:true,shiftKey:true}), "Mod+Shift+K"],
    ["mac", input({ctrlKey:true}), "Ctrl+K"],
    ["other", input({metaKey:true}), "Meta+K"],
    ["other", input({ctrlKey:true,metaKey:true}), "Ctrl+Meta+K"],
    ["mac", input({ctrlKey:true,metaKey:true}), "Ctrl+Meta+K"],
  ]) {
    const result = readRecordedShortcut(event, platform);
    assert.deepEqual(result,{kind:"binding",shortcut:expected});
    assert.equal(matchesShortcut(result.shortcut,event,platform),true);
  }
});

test("shifted punctuation uses physical codes and supports space and function keys", () => {
  const punctuation = input({key:"?",code:"Slash",ctrlKey:true,shiftKey:true});
  assert.deepEqual(readRecordedShortcut(punctuation,"other"),{kind:"binding",shortcut:"Mod+Shift+Slash"});
  assert.deepEqual(readRecordedShortcut(input({key:"+",code:"Equal",ctrlKey:true,shiftKey:true}),"other"),{kind:"binding",shortcut:"Mod+Shift+Equal"});
  assert.deepEqual(readRecordedShortcut(input({key:" ",code:"Space",ctrlKey:true}),"other"),{kind:"binding",shortcut:"Mod+Space"});
  assert.deepEqual(readRecordedShortcut(input({key:"F8",code:"F8"}),"other"),{kind:"binding",shortcut:"F8"});
});

test("modifier-only events are previews; IME, AltGraph, dead keys and held-key repeats are ignored", () => {
  assert.deepEqual(readRecordedShortcut(input({key:"Control",ctrlKey:true}),"other"),{kind:"modifiers",shortcut:"Mod"});
  assert.deepEqual(readRecordedShortcut(input({key:"Shift",ctrlKey:true,shiftKey:true}),"other"),{kind:"modifiers",shortcut:"Mod+Shift"});
  assert.deepEqual(readRecordedShortcut(input({key:"Control"}),"other"),{kind:"modifiers",shortcut:""});
  for (const event of [
    input({repeat:true,ctrlKey:true}),input({isComposing:true}),input({keyCode:229}),
    input({key:"Dead"}),input({key:"Unidentified"}),input({key:"AltGraph"}),
    input({getModifierState:key=>key==="AltGraph"}),
  ]) assert.deepEqual(readRecordedShortcut(event,"other"),{kind:"ignore"});
});

test("recording rejects typing/navigation keys and protected shortcuts using the same rules as manual input", () => {
  assert.throws(()=>readRecordedShortcut(input(),"other"),/组合键/);
  assert.throws(()=>readRecordedShortcut(input({key:"Tab",code:"Tab"}),"other"),/组合键/);
  assert.throws(()=>readRecordedShortcut(input({key:"c",ctrlKey:true,shiftKey:true}),"other"),/保留/);
  assert.throws(()=>readRecordedShortcut(input({key:"F12",code:"F12"}),"other"),/保留/);
  assert.throws(()=>readRecordedShortcut(input({key:"CapsLock",code:"CapsLock"}),"other"),/无效/);
});