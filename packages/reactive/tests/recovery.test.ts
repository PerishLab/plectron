import test from "node:test";
import assert from "node:assert/strict";
import { Scope } from "@plectron/reactive";

const checkpoint = () => { return Promise.resolve(); };

test("reset reconstructs a failed boundary as a new generation", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1, broken: false});
    const values = [];
    const failures = [];
    let retry = null;
    let stale = null;
    let setups = 0;
    const region = scope.boundary((wrap) => {
        setups += 1;
        stale = wrap(() => values[values.length] = "callback");
        scope.effect(() => {
            values[values.length] = state.value;
            if (state.broken) { throw new Error("content"); }
        });
        return setups;
    }, (error, reset) => { assert(error instanceof Error);
        failures[failures.length] = error.message;
        retry = reset;
    });
    assert.equal(region.reset(), false);
    assert.equal(region.generation, 0);
    const first = stale;
    state.broken = true;
    await checkpoint();
    assert.equal(region.failed, true);
    assert.equal(region.value, undefined);
    assert.equal(region.generation, 0);
    assert.equal(first(), undefined);
    state.broken = false;
    assert.equal(retry(), true);
    assert.equal(region.failed, false);
    assert.equal(region.generation, 1);
    assert.equal(region.value, 2);
    stale();
    state.value = 2;
    await checkpoint();
    assert.deepEqual(failures, ["content"]);
    assert.deepEqual(values, [1, 1, 1, "callback", 2]);
    scope.dispose();
});

test("a disposed boundary cannot reset", async () => {
    const scope = new Scope();
    const state = scope.reactive({broken: false});
    let retry = null;
    const region = scope.boundary(() => {
        scope.effect(() => {
            if (state.broken) { throw new Error("content"); }
        });
    }, (error, reset) => { assert(error instanceof Error); retry = reset; });
    state.broken = true;
    await checkpoint();
    region.dispose();
    assert.equal(retry(), false);
    assert.equal(region.generation, 0);
    scope.dispose();
});

test("reset cannot revive a boundary from an earlier parent run", async () => {
    const scope = new Scope();
    const state = scope.reactive({parent: 1, broken: false});
    const retries = [];
    const content = () => {
        if (state.broken) { throw new Error("content"); }
    };
    scope.effect(() => {
        state.parent;
        scope.boundary(() => {
            scope.effect(content);
        }, (error, reset) => { assert(error instanceof Error); retries[retries.length] = reset; });
    });
    state.broken = true;
    await checkpoint();
    assert.equal(retries.length, 1);
    state.parent = 2;
    await checkpoint();
    assert.equal(retries.length, 2);
    state.broken = false;
    assert.equal(retries[0](), false);
    assert.equal(retries[1](), true);
    scope.dispose();
});

test("diagnostics preserve primary and fallback failures", () => {
    const scope = new Scope();
    const reports = [];
    const outer = [];
    scope.boundary(() => {
        scope.boundary(() => {
            throw new Error("primary");
        }, () => {
            throw new Error("secondary");
        }, (failure) => { reports[reports.length] = failure; });
    }, (error) => { assert(error instanceof Error); outer[outer.length] = error.message; });
    assert.equal(reports.length, 1);
    assert.equal(reports[0].primary.message, "primary");
    assert.equal(reports[0].secondary.message, "secondary");
    assert.equal(reports[0].phase, "failed");
    assert.equal(reports[0].generation, 0);
    assert.equal(reports[0].owner.failed, true);
    assert.deepEqual(outer, ["secondary"]);
    scope.dispose();
});

test("a handled failure is diagnosed independently", () => {
    const scope = new Scope();
    const reports = [];
    scope.boundary(() => {
        throw new Error("primary");
    }, () => {}, (failure) => { reports[reports.length] = failure; });
    assert.equal(reports.length, 1);
    assert.equal(reports[0].primary.message, "primary");
    assert.equal(reports[0].secondary, undefined);
    assert.equal(reports[0].phase, "content");
    scope.dispose();
});
