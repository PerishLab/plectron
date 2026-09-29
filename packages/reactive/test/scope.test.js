import test from "node:test";
import assert from "node:assert/strict";
import { Scope } from "../src/index.js";

const checkpoint = () => Promise.resolve();

test("Scope is an explicit isolated lifecycle owner", () => {
    assert.throws(() => Scope(), TypeError);
    const first = new Scope();
    const second = new Scope();
    assert.equal(first instanceof Scope, true);
    assert.equal(first.effect, second.effect);
    const raw = {value: 1};
    assert.notEqual(first.reactive(raw), second.reactive(raw));
    const effect = first.effect;
    assert.throws(() => effect(() => {}), /invalid reactive scope/);
    assert.throws(() => Scope.prototype.reactive.call({}, raw), /invalid reactive scope/);
    first.dispose();
    second.dispose();
});

test("proxy identity and nested values are stable within a scope", () => {
    const scope = new Scope();
    const raw = {child: {value: 1}};
    const state = scope.reactive(raw);
    assert.equal(scope.reactive(raw), state);
    assert.equal(scope.reactive(state), state);
    assert.equal(state.child, state.child);
    assert.equal(scope.reactive(raw.child), state.child);
    scope.dispose();
    assert.equal(state.child, raw.child);
    assert.throws(() => scope.reactive({}), /disposed/);
});

test("fixed data properties preserve Proxy get invariants", () => {
    const scope = new Scope();
    const child = {value: 1};
    const raw = {};
    Reflect.defineProperty(raw, "fixed", {
        value: child, writable: false, configurable: false
    });
    const state = scope.reactive(raw);
    assert.equal(state.fixed, child);
    scope.dispose();
});

test("effects track reads and clear abandoned branches", async () => {
    const scope = new Scope();
    const state = scope.reactive({left: 1, right: 10, pick: true});
    const values = [];
    const stop = scope.effect(() => {
        values[values.length] = state.pick ? state.left : state.right;
    });
    assert.deepEqual(values, [1]);
    state.left = 2;
    state.left = 3;
    assert.deepEqual(values, [1]);
    await checkpoint();
    assert.deepEqual(values, [1, 3]);
    state.pick = false;
    await checkpoint();
    assert.deepEqual(values, [1, 3, 10]);
    state.left = 4;
    await checkpoint();
    assert.deepEqual(values, [1, 3, 10]);
    state.right = 11;
    await checkpoint();
    assert.deepEqual(values, [1, 3, 10, 11]);
    stop();
    state.right = 12;
    await checkpoint();
    assert.deepEqual(values, [1, 3, 10, 11]);
    scope.dispose();
});

test("existence and keys have separate dependencies", async () => {
    const scope = new Scope();
    const state = scope.reactive({item: 1});
    let has = 0;
    let keys = 0;
    scope.effect(() => { has += 1; return "item" in state; });
    scope.effect(() => { keys += 1; return Reflect.ownKeys(state); });
    state.item = 2;
    await checkpoint();
    assert.deepEqual([has, keys], [1, 1]);
    delete state.item;
    await checkpoint();
    assert.deepEqual([has, keys], [2, 2]);
    state.item = 3;
    await checkpoint();
    assert.deepEqual([has, keys], [3, 3]);
    scope.dispose();
});

test("shadowing an inherited key changes own keys without changing existence", async () => {
    const scope = new Scope();
    const raw = {};
    Reflect.setPrototypeOf(raw, {item: 1});
    const state = scope.reactive(raw);
    let has = 0;
    let keys = 0;
    scope.effect(() => { has += 1; return "item" in state; });
    scope.effect(() => { keys += 1; return Reflect.ownKeys(state); });
    state.item = 2;
    await checkpoint();
    assert.deepEqual([has, keys], [1, 2]);
    scope.dispose();
});

test("same value writes and scope disposal do not rerun effects", async () => {
    const scope = new Scope();
    const state = scope.reactive({zero: 0, value: Number.NaN});
    let runs = 0;
    scope.effect(() => { runs += 1; return [state.zero, state.value]; });
    state.zero = 0;
    state.value = Number.NaN;
    await checkpoint();
    assert.equal(runs, 1);
    state.zero = -0;
    await checkpoint();
    assert.equal(runs, 2);
    scope.dispose();
    state.zero = 2;
    await checkpoint();
    assert.equal(runs, 2);
});

test("nested effects keep their own dependencies and follow parent lifetime", async () => {
    const scope = new Scope();
    const state = scope.reactive({outer: 1, inner: 1, tail: 1});
    const trace = [];
    const stop = scope.effect(() => {
        trace[trace.length] = "outer:" + state.outer + ":" + state.tail;
        scope.effect(() => { trace[trace.length] = "inner:" + state.inner; });
    });
    assert.deepEqual(trace, ["outer:1:1", "inner:1"]);
    state.inner = 2;
    await checkpoint();
    assert.deepEqual(trace, ["outer:1:1", "inner:1", "inner:2"]);
    state.outer = 2;
    await checkpoint();
    assert.deepEqual(trace.slice(-2), ["outer:2:1", "inner:2"]);
    stop();
    state.inner = 3;
    await checkpoint();
    assert.deepEqual(trace.slice(-2), ["outer:2:1", "inner:2"]);
    scope.dispose();
});

test("an effect that throws restores tracking and releases subscriptions", async () => {
    const scope = new Scope();
    const state = scope.reactive({fault: 1, good: 1});
    assert.throws(() => scope.effect(() => {
        state.fault;
        throw new Error("boom");
    }), /boom/);
    let runs = 0;
    scope.effect(() => { runs += 1; return state.good; });
    state.fault = 2;
    await checkpoint();
    assert.equal(runs, 1);
    state.good = 2;
    await checkpoint();
    assert.equal(runs, 2);
    scope.dispose();
});

test("array value traversal differs from keys, length and direct index reads", async () => {
    const scope = new Scope();
    const items = scope.reactive([1, 2]);
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

test("computed is lazy, cached and suppresses unchanged downstream values", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    let calculations = 0;
    let effects = 0;
    const parity = scope.computed(() => {
        calculations += 1;
        return state.value % 2;
    });
    assert.equal(calculations, 0);
    assert.equal(parity.value, 1);
    assert.equal(parity.value, 1);
    assert.equal(calculations, 1);
    scope.effect(() => { effects += 1; return parity.value; });
    state.value = 3;
    assert.equal(parity.value, 1);
    assert.equal(calculations, 2);
    await checkpoint();
    assert.equal(effects, 1);
    state.value = 4;
    await checkpoint();
    assert.equal(parity.value, 0);
    assert.equal(effects, 2);
    scope.dispose();
});

test("computed diamond reaches one downstream effect per checkpoint", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    const left = scope.computed(() => state.value + 1);
    const right = scope.computed(() => state.value * 2);
    const sum = scope.computed(() => left.value + right.value);
    const values = [];
    scope.effect(() => { values[values.length] = sum.value; });
    state.value = 2;
    state.value = 3;
    await checkpoint();
    assert.deepEqual(values, [4, 10]);
    scope.dispose();
});

test("computed switches dependencies and detects cycles", async () => {
    const scope = new Scope();
    const state = scope.reactive({pick: true, left: 1, right: 5});
    const selected = scope.computed(() => state.pick ? state.left : state.right);
    const values = [];
    scope.effect(() => { values[values.length] = selected.value; });
    state.pick = false;
    await checkpoint();
    state.left = 2;
    await checkpoint();
    state.right = 6;
    await checkpoint();
    assert.deepEqual(values, [1, 5, 6]);
    let cycle = null;
    cycle = scope.computed(() => cycle.value + 1);
    assert.throws(() => cycle.value, /cycle/);
    scope.dispose();
});

test("batch returns the callback value and flushes once after nested writes", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    const values = [];
    scope.effect(() => { values[values.length] = state.value; });
    const result = scope.batch(() => {
        state.value = 2;
        scope.batch(() => { state.value = 3; });
        state.value = 4;
        return 7;
    });
    assert.equal(result, 7);
    assert.deepEqual(values, [1]);
    await checkpoint();
    assert.deepEqual(values, [1, 4]);
    assert.throws(() => scope.batch(() => {
        state.value = 5;
        throw new Error("failed");
    }), /failed/);
    await checkpoint();
    assert.deepEqual(values, [1, 4, 5]);
    scope.dispose();
});

test("untrack excludes only reads inside its callback", async () => {
    const scope = new Scope();
    const state = scope.reactive({tracked: 1, ignored: 1});
    let runs = 0;
    scope.effect(() => {
        runs += 1;
        state.tracked;
        scope.untrack(() => state.ignored);
    });
    state.ignored = 2;
    await checkpoint();
    assert.equal(runs, 1);
    state.tracked = 2;
    await checkpoint();
    assert.equal(runs, 2);
    scope.dispose();
});
