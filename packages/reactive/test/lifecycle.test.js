import test from "node:test";
import assert from "node:assert/strict";
import { Scope } from "../src/index.js";

const checkpoint = () => Promise.resolve();

test("stopping a queued effect cancels its pending ownership", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    const values = [];
    const stop = scope.effect(() => { values[values.length] = state.value; });
    state.value = 2;
    stop();
    await checkpoint();
    assert.deepEqual(values, [1]);
    scope.dispose();
});

test("disposing a scope cancels every queued unit", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    const values = [];
    scope.effect(() => { values[values.length] = state.value; });
    state.value = 2;
    scope.dispose();
    await checkpoint();
    assert.deepEqual(values, [1]);
});

test("a parent rerun replaces a queued child before it can run", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    const values = [];
    scope.effect(() => {
        values[values.length] = "parent:" + state.value;
        scope.effect(() => {
            values[values.length] = "child:" + state.value;
        });
    });
    state.value = 2;
    await checkpoint();
    assert.deepEqual(values, ["parent:1", "child:1", "parent:2", "child:2"]);
    scope.dispose();
});

test("computed children stop following dependencies after replacement", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    const values = [];
    const history = [];
    scope.effect(() => {
        const current = scope.computed(() => state.value * 2);
        history[history.length] = current;
        values[values.length] = current.value;
    });
    state.value = 2;
    await checkpoint();
    await checkpoint();
    assert.deepEqual(values, [2, 4]);
    assert.equal(history[0].value, 4);
    state.value = 3;
    await checkpoint();
    await checkpoint();
    assert.deepEqual(values, [2, 4, 6]);
    assert.equal(history[0].value, 4);
    scope.dispose();
});

test("untrack suppresses dependencies without escaping parent ownership", async () => {
    const scope = new Scope();
    const state = scope.reactive({outer: 1, inner: 1});
    const values = [];
    const stop = scope.effect(() => {
        state.outer;
        scope.untrack(() => {
            scope.effect(() => { values[values.length] = state.inner; });
        });
    });
    state.outer = 2;
    await checkpoint();
    state.inner = 2;
    await checkpoint();
    assert.deepEqual(values, [1, 1, 2]);
    stop();
    state.inner = 3;
    await checkpoint();
    assert.deepEqual(values, [1, 1, 2]);
    scope.dispose();
});
