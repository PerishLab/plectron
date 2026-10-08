import test from "node:test";
import assert from "node:assert/strict";
import { Scope } from "@plectron/reactive";

const checkpoint = () => { return Promise.resolve(); };

test("array value traversal differs from keys, length and direct index reads", async () => {
    const scope = new Scope();
    const items = scope.reactive([1, 2] as number[] & {note?: number});
    const counts = {values: 0, keys: 0, length: 0, first: 0};
    scope.effect(() => {
        counts.values += 1;
        for (const value of items) { value; }
    });
    scope.effect(() => { counts.keys += 1; Reflect.ownKeys(items); });
    scope.effect(() => { counts.length += 1; items.length; });
    scope.effect(() => { counts.first += 1; items[0]; });
    items.note = 3;
    await checkpoint();
    assert.deepEqual(counts, {values: 1, keys: 2, length: 1, first: 1});
    items[1] = 4;
    await checkpoint();
    assert.deepEqual(counts, {values: 2, keys: 2, length: 1, first: 1});
    items[2] = 5;
    await checkpoint();
    assert.deepEqual(counts, {values: 3, keys: 3, length: 2, first: 1});
    delete items[0];
    await checkpoint();
    assert.deepEqual(counts, {values: 4, keys: 4, length: 2, first: 2});
    items.length = 1;
    await checkpoint();
    assert.deepEqual(counts, {values: 5, keys: 5, length: 3, first: 2});
    scope.dispose();
});

test("consuming an iterator tracks values even when traversal breaks early", async () => {
    const scope = new Scope();
    const items = scope.reactive([1, 2, 3]);
    let consumed = 0;
    let method = 0;
    scope.effect(() => {
        consumed += 1;
        for (const value of items) { value; break; }
    });
    scope.effect(() => { method += 1; return items[Symbol.iterator]; });
    items[2] = 4;
    await checkpoint();
    assert.deepEqual([consumed, method], [2, 1]);
    scope.dispose();
});

test("shortening an array invalidates reads of removed indexes", async () => {
    const scope = new Scope();
    const items = scope.reactive([1, 2, 3]);
    const values = [];
    scope.effect(() => { values[values.length] = items[2]; });
    items.length = 1;
    await checkpoint();
    assert.deepEqual(values, [3, undefined]);
    scope.dispose();
});
