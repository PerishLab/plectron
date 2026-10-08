import { create as tracking } from "./tracking.ts";
import { create as regions } from "./boundary.ts";
import { create as scheduling } from "./scheduler.ts";
import { remove, same } from "./value.ts";
import type { Context, Handler, Report, Setup, Unit } from "./model.ts";

const states = new WeakMap<Scope, ReturnType<typeof initialize>>();

const initialize = function () {
    const root: Unit = {children: [], parent: null, live: true, epoch: 0};
    const context: Context = {active: null, owner: root, disposed: false, scheduler: null};
    const dependencies = tracking(context);
    const clear = (unit: Unit) => {
        const children = [...unit.children];
        for (const child of children) { stop(child); }
        unit.children.length = 0;
        for (const dep of unit.deps || []) { remove(dep.units, unit); }
        if (unit.deps === undefined) { return; }
        unit.deps.length = 0;
    };
    const stop = (unit: Unit) => {
        if (!unit.live) { return; }
        unit.live = false;
        if (unit.version !== undefined) { unit.version += 1; }
        context.scheduler.cancel(unit);
        clear(unit);
        if (unit.handler !== undefined) { unit.handler = undefined; }
        if (unit.parent !== null) { remove(unit.parent.children, unit); }
        unit.parent = null;
    };
    const adopt = (unit: Unit, parent: Unit) => {
        unit.parent = parent;
        parent.children[parent.children.length] = unit;
    };
    const run = (unit: Unit) => {
        if (!unit.live || context.disposed) { return; }
        clear(unit);
        unit.epoch += 1;
        const listener = context.active;
        const parent = context.owner;
        context.active = unit;
        context.owner = unit;
        try { return unit.fn(); }
        finally {
            context.active = listener;
            context.owner = parent;
        }
    };
    const compute = (unit: Unit) => {
        if (unit.running) { throw new Error("computed cycle"); }
        unit.running = true;
        const previous = unit.value;
        const ready = unit.ready;
        try {
            const value = run(unit);
            unit.value = value;
            unit.ready = true;
            unit.dirty = false;
            if (ready && !same(previous, value)) {
                dependencies.notify(unit.entry, "get", "value");
            }
        } finally { unit.running = false; }
    };
    const boundaries = regions(context, {stop: stop, adopt: adopt});
    context.scheduler = scheduling((unit) => {
        if (!unit.live) { return; }
        if (unit.computed) {
            if (unit.dirty) { compute(unit); }
        } else { run(unit); }
    }, (unit, error) => boundaries.capture(unit, error, true) !== null);
    const effect = (fn: () => unknown) => {
        if (context.disposed) { throw new Error("reactive scope is disposed"); }
        const unit: Unit = {
            fn: fn, deps: [], children: [], parent: null,
            live: true, epoch: 0
        };
        adopt(unit, context.owner);
        try { run(unit); }
        catch (error) { stop(unit); throw error; }
        return () => { stop(unit); };
    };
    const computed = <T>(fn: () => T) => {
        if (context.disposed) { throw new Error("reactive scope is disposed"); }
        const entry = {deps: []};
        const unit: Unit = {
            fn: fn, deps: [], children: [], parent: null, live: true, computed: true,
            entry: entry, ready: false, dirty: true, running: false, value: undefined,
            epoch: 0
        };
        adopt(unit, context.owner);
        return {
            get value(): T | undefined {
                if (unit.running) { throw new Error("computed cycle"); }
                dependencies.track(entry, "get", "value");
                if (unit.dirty && unit.live && !context.disposed) { compute(unit); }
                return unit.value as T | undefined;
            }
        };
    };
    const batch = <T>(fn: () => T): T => {
        if (context.disposed) { throw new Error("reactive scope is disposed"); }
        return context.scheduler.batch(fn);
    };
    const untrack = <T>(fn: () => T): T => {
        const listener = context.active;
        context.active = null;
        try { return fn(); } finally { context.active = listener; }
    };
    const dispose = () => {
        if (context.disposed) { return; }
        const children = [...root.children];
        for (const unit of children) { stop(unit); }
        root.children.length = 0;
        context.scheduler.dispose();
        dependencies.dispose();
        context.disposed = true;
    };
    const wrap = <T>(value: T): T => {
        if (context.disposed) { throw new Error("reactive scope is disposed"); }
        return dependencies.reactive(value);
    };
    return {
        reactive: wrap, effect: effect, computed: computed,
        batch: batch, untrack: untrack, boundary: boundaries.boundary, dispose: dispose
    };
};

const state = (scope: Scope) => {
    const value = states.get(scope);
    if (value === undefined) { throw new TypeError("invalid reactive scope"); }
    return value;
};

class Scope {
    constructor() { states.set(this, initialize()); }
    reactive<T>(value: T): T { return state(this).reactive(value); }
    effect(fn: () => unknown) { return state(this).effect(fn); }
    computed<T>(fn: () => T) { return state(this).computed(fn); }
    batch<T>(fn: () => T): T { return state(this).batch(fn); }
    untrack<T>(fn: () => T): T { return state(this).untrack(fn); }
    boundary<T>(fn: Setup<T>, handler: Handler, report?: Report) {
        return state(this).boundary(fn, handler, report);
    }
    dispose() { return state(this).dispose(); }
}

export { Scope };
