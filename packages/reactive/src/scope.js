import { createArray } from "./array.js";
import { create as scheduling } from "./scheduler.js";

const same = (left, right) => {
    if (left === right) { return left !== 0 || 1 / left === 1 / right; }
    return left !== left && right !== right;
};

const remove = (items, value) => {
    let found = false;
    let index = 0;
    for (const item of items) {
        if (!found && item === value) { found = true; continue; }
        if (found) { items[index] = item; }
        index += 1;
    }
    if (found) { items.length -= 1; }
};

const states = new WeakMap();

const initialize = function () {
    let entries = new WeakMap();
    const root = {children: [], parent: null, live: true, epoch: 0};
    let active = null;
    let owner = root;
    let disposed = false;
    let scheduler = null;

    const unwrap = (value) => {
        const entry = entries.get(value);
        if (entry !== undefined && entry.proxy === value) { return entry.raw; }
        return value;
    };
    const slot = (entry, kind, key) => {
        for (const dep of entry.deps) {
            if (dep.kind === kind && dep.key === key) { return dep; }
        }
        const dep = {kind: kind, key: key, units: []};
        entry.deps[entry.deps.length] = dep;
        return dep;
    };
    const track = (entry, kind, key) => {
        if (active === null || disposed) { return; }
        const dep = slot(entry, kind, key);
        for (const unit of dep.units) {
            if (unit === active) { return; }
        }
        dep.units[dep.units.length] = active;
        active.deps[active.deps.length] = dep;
    };
    const clear = (unit) => {
        const children = [...unit.children];
        for (const child of children) { stop(child); }
        unit.children.length = 0;
        for (const dep of unit.deps || []) { remove(dep.units, unit); }
        if (unit.deps === undefined) { return; }
        unit.deps.length = 0;
    };
    const stop = (unit) => {
        if (!unit.live) { return; }
        unit.live = false;
        if (unit.version !== undefined) { unit.version += 1; }
        scheduler.cancel(unit);
        clear(unit);
        if (unit.handler !== undefined) { unit.handler = undefined; }
        if (unit.parent !== null) { remove(unit.parent.children, unit); }
        unit.parent = null;
    };
    const attach = (unit) => {
        unit.parent = owner;
        owner.children[owner.children.length] = unit;
    };
    const adopt = (unit, parent) => {
        unit.parent = parent;
        parent.children[parent.children.length] = unit;
    };
    const run = (unit) => {
        if (!unit.live || disposed) { return; }
        clear(unit);
        unit.epoch += 1;
        const listener = active;
        const parent = owner;
        active = unit;
        owner = unit;
        try { return unit.fn(); }
        finally {
            active = listener;
            owner = parent;
        }
    };
    const compute = (unit) => {
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
                notify(unit.entry, "get", "value");
            }
        } finally { unit.running = false; }
    };
    const nearest = (unit) => {
        if (unit === null || unit.handler !== undefined) { return unit; }
        return nearest(unit.parent);
    };
    const diagnose = (region, failure) => {
        if (region.report === undefined) { return; }
        try { region.report(failure); }
        catch (error) { Promise.reject(error); }
    };
    const capture = (unit, error, climb) => {
        const region = nearest(unit);
        if (region === null) { return null; }
        if (region.failed) { return region; }
        region.failed = true;
        const handler = region.handler;
        const source = region.parent;
        const control = region.control;
        const failure = {
            primary: error, secondary: undefined,
            phase: "content", generation: region.generation,
            owner: control
        };
        control.failed = true;
        control.value = undefined;
        stop(region);
        const listener = active;
        const parent = owner;
        active = null;
        owner = source;
        try {
            handler(error, control.reset, failure);
            diagnose(region, failure);
        }
        catch (next) {
            diagnose(region, {
                primary: error, secondary: next,
                phase: "failed", generation: region.generation,
                owner: control
            });
            if (!climb) { throw next; }
            const caught = capture(source, next, true);
            if (caught === null) { throw next; }
            return caught;
        } finally {
            active = listener;
            owner = parent;
        }
        return region;
    };
    scheduler = scheduling((unit) => {
        if (!unit.live) { return; }
        if (unit.computed) {
            if (unit.dirty) { compute(unit); }
        } else { run(unit); }
    }, (unit, error) => capture(unit, error, true) !== null);
    const observed = (unit) => {
        for (const dep of unit.entry.deps) {
            if (dep.units.length > 0) { return true; }
        }
        return false;
    };
    const notify = (entry, kind, key) => {
        for (const dep of entry.deps) {
            if (dep.kind !== kind || dep.key !== key) { continue; }
            const pending = [...dep.units];
            for (const unit of pending) {
                if (!unit.live || unit === active) { continue; }
                if (unit.computed) {
                    unit.dirty = true;
                    if (unit.ready && observed(unit)) { scheduler.enqueue(unit); }
                } else { scheduler.enqueue(unit); }
            }
        }
    };
    const changed = (before, after) => {
        if (before === undefined || after === undefined) { return true; }
        if (!("value" in before) || !("value" in after)) { return true; }
        return !same(before.value, after.value);
    };
    const publish = (entry, key, before, after, existed, exists) => {
        if (changed(before, after)) { notify(entry, "get", key); }
        if (existed !== exists) { notify(entry, "has", key); }
        if ((before === undefined) !== (after === undefined)) {
            notify(entry, "keys", undefined);
        }
    };
    const array = createArray(track, notify, changed, unwrap);
    const reactive = (value) => {
        if (value === null || typeof value !== "object") { return value; }
        const existing = entries.get(value);
        if (existing !== undefined) { return existing.proxy; }
        if (disposed) { return value; }
        const entry = {
            raw: value, proxy: null, deps: [], writing: 0,
            array: Array.isArray(value), method: null, iterate: null
        };
        const proxy = new Proxy(value, {
            get(target, key, receiver) {
                const result = Reflect.get(target, key, receiver);
                track(entry, "get", key);
                const descriptor = Reflect.getOwnPropertyDescriptor(target, key);
                if (descriptor !== undefined && !descriptor.configurable
                    && descriptor.writable === false) { return result; }
                if (entry.array && key === Symbol.iterator && typeof result === "function") {
                    if (entry.method !== result) {
                        entry.method = result;
                        entry.iterate = array.iterate(entry, result);
                    }
                    return entry.iterate;
                }
                return reactive(result);
            },
            has(target, key) {
                const result = Reflect.has(target, key);
                track(entry, "has", key);
                return result;
            },
            ownKeys(target) {
                const result = Reflect.ownKeys(target);
                track(entry, "keys", undefined);
                return result;
            },
            set(target, key, next, receiver) {
                const before = Reflect.getOwnPropertyDescriptor(target, key);
                const existed = Reflect.has(target, key);
                const previous = array.length(entry, target);
                entry.writing += 1;
                let success = false;
                try { success = Reflect.set(target, key, unwrap(next), receiver); }
                finally { entry.writing -= 1; }
                if (success && unwrap(receiver) === target) {
                    const after = Reflect.getOwnPropertyDescriptor(target, key);
                    publish(entry, key, before, after, existed, Reflect.has(target, key));
                    array.change(entry, key, before, after, previous, target.length);
                }
                return success;
            },
            defineProperty(target, key, descriptor) {
                const before = Reflect.getOwnPropertyDescriptor(target, key);
                const existed = Reflect.has(target, key);
                const previous = array.length(entry, target);
                const success = Reflect.defineProperty(target, key, descriptor);
                if (success && entry.writing === 0) {
                    const after = Reflect.getOwnPropertyDescriptor(target, key);
                    publish(entry, key, before, after, existed, Reflect.has(target, key));
                    array.change(entry, key, before, after, previous, target.length);
                }
                return success;
            },
            deleteProperty(target, key) {
                const before = Reflect.getOwnPropertyDescriptor(target, key);
                const existed = Reflect.has(target, key);
                const previous = array.length(entry, target);
                const success = Reflect.deleteProperty(target, key);
                if (success && before !== undefined) {
                    publish(entry, key, before, undefined, existed, Reflect.has(target, key));
                    array.change(entry, key, before, undefined, previous, target.length);
                }
                return success;
            }
        });
        entry.proxy = proxy;
        entries.set(value, entry);
        entries.set(proxy, entry);
        return proxy;
    };
    const effect = (fn) => {
        if (disposed) { throw new Error("reactive scope is disposed"); }
        const unit = {
            fn: fn, deps: [], children: [], parent: null,
            live: true, epoch: 0
        };
        attach(unit);
        try { run(unit); }
        catch (error) { stop(unit); throw error; }
        return () => { stop(unit); };
    };
    const computed = (fn) => {
        if (disposed) { throw new Error("reactive scope is disposed"); }
        const entry = {deps: []};
        const unit = {
            fn: fn, deps: [], children: [], parent: null, live: true, computed: true,
            entry: entry, ready: false, dirty: true, running: false, value: undefined,
            epoch: 0
        };
        attach(unit);
        return {
            get value() {
                if (unit.running) { throw new Error("computed cycle"); }
                track(entry, "get", "value");
                if (unit.dirty && unit.live && !disposed) { compute(unit); }
                return unit.value;
            }
        };
    };
    const batch = (fn) => {
        if (disposed) { throw new Error("reactive scope is disposed"); }
        return scheduler.batch(fn);
    };
    const untrack = (fn) => {
        const listener = active;
        active = null;
        try { return fn(); } finally { active = listener; }
    };
    const observe = (region, version, value) => {
        if (value === null || typeof value !== "object"
            || typeof value.catch !== "function") { return value; }
        value.catch((error) => {
            if (!region.live || region.version !== version) { throw error; }
            const caught = capture(region, error, true);
            if (caught === null) { throw error; }
        });
        return value;
    };
    const bind = (region, fn) => {
        const version = region.version;
        return (...args) => {
            if (disposed || !region.live || region.version !== version) { return; }
            const listener = active;
            const parent = owner;
            active = null;
            owner = region;
            let value = undefined;
            try { value = fn(...args); }
            catch (error) {
                const caught = capture(region, error, true);
                if (caught === null) { throw error; }
            } finally {
                active = listener;
                owner = parent;
            }
            try { return observe(region, version, value); }
            catch (error) {
                const caught = capture(region, error, true);
                if (caught === null) { throw error; }
            }
        };
    };
    const boundary = (fn, handler, report) => {
        if (disposed) { throw new Error("reactive scope is disposed"); }
        const anchor = owner;
        const epoch = anchor.epoch;
        let setup = fn;
        let fallback = handler;
        let observer = report;
        const control = {
            value: undefined, failed: false, generation: 0,
            reset: null, dispose: null
        };
        let region = null;
        let retired = false;
        const launch = (climb) => {
            if (retired || disposed || !anchor.live) { return false; }
            region = {
                children: [], parent: null, live: true, failed: false,
                handler: fallback, report: observer, version: control.generation,
                generation: control.generation, control: control, epoch: 0
            };
            adopt(region, anchor);
            control.failed = false;
            control.value = undefined;
            const parent = owner;
            owner = region;
            try { control.value = setup((callback) => bind(region, callback)); }
            catch (error) {
                const caught = capture(region, error, climb);
                if (caught === null) { throw error; }
            } finally { owner = parent; }
            return true;
        };
        control.reset = () => {
            if (retired || disposed || !anchor.live || anchor.epoch !== epoch
                || !control.failed) { return false; }
            if (region !== null && region.live) { stop(region); }
            control.generation += 1;
            return launch(true);
        };
        control.dispose = () => {
            if (retired) { return; }
            retired = true;
            if (region !== null) { stop(region); }
            control.value = undefined;
            region = null;
            setup = null;
            fallback = null;
            observer = null;
        };
        launch(false);
        return control;
    };
    const dispose = () => {
        if (disposed) { return; }
        const children = [...root.children];
        for (const unit of children) { stop(unit); }
        root.children.length = 0;
        scheduler.dispose();
        entries = new WeakMap();
        disposed = true;
    };
    const wrap = (value) => {
        if (disposed) { throw new Error("reactive scope is disposed"); }
        return reactive(value);
    };
    return {
        reactive: wrap, effect: effect, computed: computed,
        batch: batch, untrack: untrack, boundary: boundary, dispose: dispose
    };
};

const state = (scope) => {
    const value = states.get(scope);
    if (value === undefined) { throw new TypeError("invalid reactive scope"); }
    return value;
};

class Scope {
    constructor() { states.set(this, initialize()); }
    reactive(value) { return state(this).reactive(value); }
    effect(fn) { return state(this).effect(fn); }
    computed(fn) { return state(this).computed(fn); }
    batch(fn) { return state(this).batch(fn); }
    untrack(fn) { return state(this).untrack(fn); }
    boundary(fn, handler, report) {
        return state(this).boundary(fn, handler, report);
    }
    dispose() { return state(this).dispose(); }
}

export { Scope };
