import test from "node:test";
import assert from "node:assert/strict";

let notify = null;
let released = 0;
Object.defineProperty(Symbol, "plectron", {value: Symbol("plectron")});
Symbol[Symbol.plectron] = {
    visual: {
        observe: (host, callback, margin) => {
            assert.equal(margin, 64);
            notify = callback;
            return {dispose: () => { released += 1; }};
        }
    }
};
const {default: Visual} = await import("../../src/visual.js");

const environment = () => ({
    children: [],
    appendChild(node) {
        const existing = this.children.indexOf(node);
        if (existing >= 0) { this.children.splice(existing, 1); }
        this.children[this.children.length] = node;
    },
    removeChild(node) {
        const index = this.children.indexOf(node);
        if (index < 0) { throw new Error("missing child"); }
        this.children.splice(index, 1);
    }
});

test("an end window expands from exact coverage", () => {
    const host = environment();
    const retired = [];
    const visual = Visual(host, {
        count: () => 100,
        key: (index) => "item-" + index,
        mount: (index) => ({host: {index: index}, dispose: () => { retired[retired.length] = index; }}),
        batch: 4,
        margin: 64,
        anchor: "end"
    });
    assert.deepEqual(visual.range(), {start: 96, end: 100, total: 100});
    assert.deepEqual(host.children.map((node) => node.index), [96, 97, 98, 99]);
    notify({leading: 24, trailing: 0, items: 4, start: 2, end: 4});
    assert.deepEqual(visual.range(), {start: 92, end: 100, total: 100});
    assert.deepEqual(host.children.map((node) => node.index), [92, 93, 94, 95, 96, 97, 98, 99]);
    notify({leading: 128, trailing: 0, items: 8, start: 2, end: 8});
    assert.deepEqual(visual.range(), {start: 92, end: 100, total: 100});
    visual.dispose();
    assert.equal(released, 1);
    assert.equal(retired.length, 8);
});

test("zero progress violates the visual protocol", () => {
    const host = environment();
    Visual(host, {
        count: () => 20,
        mount: (index) => ({host: {index: index}}),
        batch: 4,
        margin: 64,
        anchor: "start"
    });
    notify({leading: 0, trailing: -64, items: 4, start: 0, end: 4});
    assert.throws(
        () => notify({leading: 0, trailing: -64, items: 8, start: 0, end: 8}),
        /no trailing progress/
    );
});

test("retirement crosses hysteresis in whole batches", () => {
    const host = environment();
    const visual = Visual(host, {
        count: () => 100,
        mount: (index) => ({host: {index: index}}),
        batch: 4,
        margin: 64,
        anchor: "start"
    });
    notify({leading: 0, trailing: -100, items: 4, start: 0, end: 4});
    notify({leading: 100, trailing: -50, items: 8, start: 4, end: 8});
    notify({leading: 100, trailing: 100, items: 12, start: 5, end: 12});
    assert.deepEqual(visual.range(), {start: 0, end: 12, total: 100});
    notify({leading: 100, trailing: 100, items: 12, start: 8, end: 12});
    assert.deepEqual(visual.range(), {start: 4, end: 12, total: 100});
    assert.deepEqual(host.children.map((node) => node.index), [4, 5, 6, 7, 8, 9, 10, 11]);
});

test("layout observations preserve the current semantic anchor", () => {
    const host = environment();
    const bookmarks = [];
    const scrolling = {
        restore: (bookmark) => {
            bookmarks[bookmarks.length] = bookmark;
            return true;
        }
    };
    Visual(host, {
        count: () => 100,
        key: (index) => "row-" + index,
        mount: (index) => ({host: {index: index}}),
        batch: 4,
        margin: 64,
        anchor: "start",
        scroll: scrolling
    });
    notify({leading: 0, trailing: 100, items: 4, start: 0, end: 4, inset: 0});
    bookmarks.length = 0;
    notify({leading: 20, trailing: -100, items: 4, start: 1, end: 4, inset: -12});
    assert.deepEqual(bookmarks, [{edge: "anchor", key: "row-1", inset: -12, fallbacks: []}]);
});
