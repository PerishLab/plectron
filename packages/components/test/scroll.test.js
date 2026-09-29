import test from "node:test";
import assert from "node:assert/strict";

const calls = [];
let listener = null;
const controller = {
    followEnd: () => { calls[calls.length] = ["end"]; },
    restore: (target, inset) => { calls[calls.length] = ["restore", target, inset]; },
    dispose: () => { calls[calls.length] = ["dispose"]; }
};
const scope = {dispose: () => { calls[calls.length] = ["scope"]; }};
Object.defineProperty(Symbol, "plectron", {value: Symbol("plectron")});
Symbol[Symbol.plectron] = {
    events: {scope: () => scope},
    scroll: {attach: () => controller}
};
const {default: Scroll} = await import("../src/scroll.js");

const host = {
    rows: [],
    children() { return this.rows; },
    addEventListener: (name, callback) => {
        assert.equal(name, "scroll");
        listener = callback;
    }
};

test("semantic bookmarks survive replacement nodes", () => {
    const states = [];
    const scrolling = Scroll(host, (state) => { states[states.length] = state; }, (node) => node.id);
    const original = {id: "message-7"};
    host.rows = [original];
    listener({scroll: {edge: "middle", position: {target: original, inset: -12, fallbacks: []}}});
    const bookmark = scrolling.bookmark();
    assert.deepEqual(bookmark, {edge: "anchor", key: "message-7", inset: -12, fallbacks: []});
    const replacement = {id: "message-7"};
    host.rows = [replacement];
    assert.equal(scrolling.restore(bookmark), true);
    assert.deepEqual(calls.pop(), ["restore", replacement, -12]);
    assert.equal(states.length, 1);
});

test("end and missing anchors have explicit fallbacks", () => {
    const scrolling = Scroll(host, undefined, (node) => node.id);
    host.rows = [];
    listener({scroll: {edge: "end", position: null}});
    const bookmark = scrolling.bookmark();
    assert.deepEqual(bookmark, {edge: "end"});
    assert.equal(scrolling.restore(bookmark), true);
    assert.deepEqual(calls.pop(), ["end"]);
    assert.equal(scrolling.restore({edge: "anchor", key: "gone", inset: 0, fallbacks: []}), false);
    scrolling.dispose();
    assert.deepEqual(calls.splice(-2), [["scope"], ["dispose"]]);
});

test("a missing anchor selects the next item and then the previous item", () => {
    const scrolling = Scroll(host, undefined, (node) => node.id);
    const before = {id: "before"};
    const anchor = {id: "anchor"};
    const after = {id: "after"};
    host.rows = [before, anchor, after];
    listener({scroll: {edge: "middle", position: {
        target: anchor,
        inset: -8,
        fallbacks: [{target: after, inset: 72}, {target: before, inset: -88}]
    }}});
    const bookmark = scrolling.bookmark();
    host.rows = [before, after];
    assert.equal(scrolling.restore(bookmark), true);
    assert.deepEqual(calls.pop(), ["restore", after, 72]);
    host.rows = [before];
    assert.equal(scrolling.restore(bookmark), true);
    assert.deepEqual(calls.pop(), ["restore", before, -88]);
});
