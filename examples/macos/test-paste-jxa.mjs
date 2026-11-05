// test-paste-jxa.mjs — 實際執行 markdown-fix-paste.sh 裡「等修飾鍵放開再貼上」的 JXA。
//
// 從腳本抽出 <paste-jxa> 與 </paste-jxa> 之間的程式，在 node:vm 裡以模擬的
// CGEventSourceFlagsState、Date.now、delay 與 System Events 執行，不碰真正的桌面：
//
//   node --test examples/macos/test-paste-jxa.mjs
//
// test-markdown-fix-paste.sh 在有 Node 時會自動執行這支測試。

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const here = path.dirname(fileURLToPath(import.meta.url));
const script = fs.readFileSync(path.join(here, 'markdown-fix-paste.sh'), 'utf8');
const begin = script.indexOf('// <paste-jxa>');
const end = script.indexOf('// </paste-jxa>');
assert.ok(begin >= 0 && end > begin, 'markdown-fix-paste.sh 必須保留 <paste-jxa> 標記');
const jxa = script.slice(begin, end);

const SHIFT = 0x20000;
const CONTROL = 0x40000;
const OPTION = 0x80000;
const COMMAND = 0x100000;
const CAPS_LOCK = 0x10000;
const NON_COALESCED = 0x100;

// flagsAt(ms) 回傳該時間點的修飾鍵狀態。回傳 run() 的結果、送出的按鍵與結束時間。
function simulate(flagsAt, timeoutMs) {
  let now = 0;
  const keystrokes = [];
  const context = {
    ObjC: { import() {} },
    // JXA 的 ObjC bridge 會回傳數字；腳本以 Number() 包住，這裡刻意回傳字串測試這一點。
    $: { CGEventSourceFlagsState: (state) => { assert.equal(state, 0); return String(flagsAt(now)); } },
    Date: { now: () => now },
    Number,
    delay: (seconds) => { now += Math.round(seconds * 1000); },
    Application: (name) => {
      assert.equal(name, 'System Events');
      return { keystroke: (key, options) => keystrokes.push({ key, using: options.using, at: now, flags: flagsAt(now) }) };
    },
  };
  vm.createContext(context);
  vm.runInContext(jxa, context);
  const result = vm.runInContext(`run(${JSON.stringify([String(timeoutMs)])})`, context);
  return { result, keystrokes, elapsed: now };
}

test('沒有按著修飾鍵時立即貼上', () => {
  const { result, keystrokes } = simulate(() => 0, 1000);
  assert.equal(result, 'pasted');
  assert.deepEqual(keystrokes.map((k) => [k.key, k.using]), [['v', 'command down']]);
  assert.equal(keystrokes[0].flags, 0);
});

test('等到 Option＋Shift 放開才貼上', () => {
  const { result, keystrokes } = simulate((t) => (t < 300 ? OPTION | SHIFT : 0), 1000);
  assert.equal(result, 'pasted');
  assert.equal(keystrokes.length, 1);
  assert.ok(keystrokes[0].at >= 300, `在 ${keystrokes[0].at} ms 就貼上`);
  assert.equal(keystrokes[0].flags, 0);
});

test('逾時後修飾鍵仍按著就不貼上', () => {
  for (const held of [OPTION | SHIFT, CONTROL, COMMAND, SHIFT]) {
    const { result, keystrokes, elapsed } = simulate(() => held, 1000);
    assert.equal(result, 'modifiers-held');
    assert.deepEqual(keystrokes, [], `flags 0x${held.toString(16)} 時不該送出按鍵`);
    assert.ok(elapsed >= 1000 && elapsed < 1100, `等了 ${elapsed} ms`);
  }
});

test('逾時為 0 且修飾鍵按著時不貼上', () => {
  const { result, keystrokes } = simulate(() => OPTION, 0);
  assert.equal(result, 'modifiers-held');
  assert.deepEqual(keystrokes, []);
});

test('修飾鍵恰好在逾時前放開仍會貼上', () => {
  const { result, keystrokes } = simulate((t) => (t < 990 ? SHIFT : 0), 1000);
  assert.equal(result, 'pasted');
  assert.equal(keystrokes.length, 1);
});

test('Caps Lock 與 non-coalesced 旗標不算修飾鍵', () => {
  const { result, keystrokes } = simulate(() => CAPS_LOCK | NON_COALESCED, 1000);
  assert.equal(result, 'pasted');
  assert.equal(keystrokes.length, 1);
  assert.equal(keystrokes[0].at, 30);
});
