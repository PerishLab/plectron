import test from "node:test";
import assert from "node:assert/strict";
import { Scope } from "../src/index.js";

const checkpoint = () => Promise.resolve();

test("a boundary returns setup value and owns its reactive subtree", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    const values = [];
    const region = scope.boundary(() => {
        scope.effect(() => { values[values.length] = state.value; });
        return 7;
    }, () => {});
    assert.equal(region.value, 7);
    state.value = 2;
    await checkpoint();
    assert.deepEqual(values, [1, 2]);
    region.dispose();
    state.value = 3;
    await checkpoint();
    assert.deepEqual(values, [1, 2]);
    scope.dispose();
});

test("a scheduled failure retires its boundary while siblings continue", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    const first = [];
    const second = [];
    const sibling = [];
    const errors = [];
    scope.boundary(() => {
        scope.effect(() => {
            const value = state.value;
            first[first.length] = value;
            if (value === 2) { throw new Error("inside"); }
        });
        scope.effect(() => { second[second.length] = state.value; });
    }, (error) => { errors[errors.length] = error.message; });
    scope.effect(() => { sibling[sibling.length] = state.value; });
    state.value = 2;
    await checkpoint();
    assert.deepEqual(errors, ["inside"]);
    assert.deepEqual(first, [1, 2]);
    assert.deepEqual(second, [1]);
    assert.deepEqual(sibling, [1, 2]);
    state.value = 3;
    await checkpoint();
    assert.deepEqual(first, [1, 2]);
    assert.deepEqual(second, [1]);
    assert.deepEqual(sibling, [1, 2, 3]);
    scope.dispose();
});

test("a failing handler propagates to its nearest parent boundary", () => {
    const scope = new Scope();
    const errors = [];
    const trace = [];
    scope.boundary(() => {
        scope.boundary(() => {
            throw new Error("source");
        }, () => { throw new Error("handler"); });
        trace[trace.length] = "escaped";
    }, (error) => { errors[errors.length] = error.message; });
    assert.deepEqual(errors, ["handler"]);
    assert.deepEqual(trace, []);
    scope.dispose();
});

test("an asynchronous handler failure propagates through owner ancestry", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    const errors = [];
    scope.boundary(() => {
        scope.boundary(() => {
            scope.effect(() => {
                if (state.value === 2) { throw new Error("source"); }
            });
        }, () => { throw new Error("handler"); });
    }, (error) => { errors[errors.length] = error.message; });
    state.value = 2;
    await checkpoint();
    assert.deepEqual(errors, ["handler"]);
    scope.dispose();
});

test("an initial failure is handled once and leaves no subscriptions", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    const errors = [];
    scope.boundary(() => {
        scope.effect(() => {
            state.value;
            throw new Error("initial");
        });
    }, (error) => { errors[errors.length] = error.message; });
    state.value = 2;
    await checkpoint();
    assert.deepEqual(errors, ["initial"]);
    scope.dispose();
});

test("wrapped callbacks restore ownership without restoring tracking", async () => {
    const scope = new Scope();
    const state = scope.reactive({outer: 1, inner: 1});
    const values = [];
    let invoke = null;
    const region = scope.boundary((wrap) => {
        invoke = wrap((value) => {
            state.inner;
            scope.effect(() => { values[values.length] = state.inner; });
            return value * 2;
        });
    }, () => {});
    let runs = 0;
    scope.effect(() => {
        runs += 1;
        state.outer;
        assert.equal(invoke(3), 6);
    });
    state.inner = 2;
    await checkpoint();
    assert.equal(runs, 1);
    assert.deepEqual(values, [1, 2]);
    region.dispose();
    state.inner = 3;
    await checkpoint();
    assert.deepEqual(values, [1, 2]);
    assert.equal(invoke(4), undefined);
    scope.dispose();
});

test("wrapped callbacks route returned promise rejections", async () => {
    const scope = new Scope();
    const errors = [];
    let invoke = null;
    scope.boundary((wrap) => {
        invoke = wrap(async () => {
            await checkpoint();
            throw new Error("async callback");
        });
    }, (error) => { errors[errors.length] = error.message; });
    await assert.rejects(invoke(), /async callback/);
    await checkpoint();
    assert.deepEqual(errors, ["async callback"]);
    scope.dispose();
});

test("work after await does not inherit the wrapped owner", async () => {
    const scope = new Scope();
    const state = scope.reactive({value: 1});
    const values = [];
    let invoke = null;
    const region = scope.boundary((wrap) => {
        invoke = wrap(async () => {
            await checkpoint();
            scope.effect(() => { values[values.length] = state.value; });
        });
    }, () => {});
    await invoke();
    region.dispose();
    state.value = 2;
    await checkpoint();
    assert.deepEqual(values, [1, 2]);
    scope.dispose();
});

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
    }, (error, reset) => {
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
    }, (error, reset) => { retry = reset; });
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
    scope.effect(() => {
        state.parent;
        scope.boundary(() => {
            scope.effect(() => {
                if (state.broken) { throw new Error("content"); }
            });
        }, (error, reset) => { retries[retries.length] = reset; });
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
    }, (error) => { outer[outer.length] = error.message; });
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
