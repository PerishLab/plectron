import { create as arrays } from "./array.ts";
import { same } from "./value.ts";
import type { Context, Entry, Kind, Mutation, Proxy as Instance, Unit } from "./model.ts";

export const changed = (before: PropertyDescriptor, after: PropertyDescriptor) => {
    if (before === undefined || after === undefined) { return true; }
    if (!("value" in before) || !("value" in after)) { return true; }
    return !same(before.value, after.value);
};

export const create = (context: Context) => {
    let entries = new WeakMap<object, Instance>();
    const unwrap = <T>(value: T): T => {
        const entry = entries.get(value as object);
        if (entry !== undefined && entry.proxy === value) { return entry.raw as T; }
        return value;
    };
    const slot = (entry: Entry, kind: Kind, key: PropertyKey) => {
        for (const dep of entry.deps) {
            if (dep.kind === kind && dep.key === key) { return dep; }
        }
        const dep = {kind: kind, key: key, units: [] as Unit[]};
        entry.deps[entry.deps.length] = dep;
        return dep;
    };
    const track = (entry: Entry, kind: Kind, key: PropertyKey) => {
        const active = context.active;
        if (active === null || context.disposed) { return; }
        const dep = slot(entry, kind, key);
        for (const unit of dep.units) {
            if (unit === active) { return; }
        }
        dep.units[dep.units.length] = active;
        active.deps[active.deps.length] = dep;
    };
    const observed = (unit: Unit) => {
        for (const dep of unit.entry.deps) {
            if (dep.units.length > 0) { return true; }
        }
        return false;
    };
    const invalidate = (unit: Unit) => {
        if (!unit.live || unit === context.active) { return; }
        if (!unit.computed) { context.scheduler.enqueue(unit); return; }
        unit.dirty = true;
        if (unit.ready && observed(unit)) { context.scheduler.enqueue(unit); }
    };
    const notify = (entry: Entry, kind: Kind, key: PropertyKey) => {
        for (const dep of entry.deps) {
            if (dep.kind !== kind || dep.key !== key) { continue; }
            const pending = [...dep.units];
            for (const unit of pending) {
                invalidate(unit);
            }
        }
    };
    const publish = (entry: Entry, mutation: Mutation) => {
        if (changed(mutation.before, mutation.after)) { notify(entry, "get", mutation.key); }
        if (mutation.existed !== mutation.exists) { notify(entry, "has", mutation.key); }
        if ((mutation.before === undefined) !== (mutation.after === undefined)) {
            notify(entry, "keys", undefined);
        }
    };
    const array = arrays(track, notify, changed, unwrap);
    const iteration = (entry: Instance, method: Function) => {
        if (entry.method !== method) {
            entry.method = method;
            entry.iterate = array.iterate(entry, method);
        }
        return entry.iterate;
    };
    const reactive = <T>(value: T): T => {
        if (value === null || typeof value !== "object") { return value; }
        const existing = entries.get(value);
        if (existing !== undefined) { return existing.proxy as T; }
        if (context.disposed) { return value; }
        const entry: Instance = {
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
                    return iteration(entry, result);
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
                    const mutation = {key: key, before: before, after: after, existed: existed,
                        exists: Reflect.has(target, key), previous: previous, length: undefined as number};
                    publish(entry, mutation);
                    mutation.length = Reflect.get(target, "length") as number;
                    array.change(entry, mutation);
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
                    const mutation = {key: key, before: before, after: after, existed: existed,
                        exists: Reflect.has(target, key), previous: previous, length: undefined as number};
                    publish(entry, mutation);
                    mutation.length = Reflect.get(target, "length") as number;
                    array.change(entry, mutation);
                }
                return success;
            },
            deleteProperty(target, key) {
                const before = Reflect.getOwnPropertyDescriptor(target, key);
                const existed = Reflect.has(target, key);
                const previous = array.length(entry, target);
                const success = Reflect.deleteProperty(target, key);
                if (success && before !== undefined) {
                    const mutation = {key: key, before: before, after: undefined, existed: existed,
                        exists: Reflect.has(target, key), previous: previous,
                        length: undefined as number};
                    publish(entry, mutation);
                    mutation.length = Reflect.get(target, "length") as number;
                    array.change(entry, mutation);
                }
                return success;
            }
        });
        entry.proxy = proxy;
        entries.set(value, entry);
        entries.set(proxy, entry);
        return proxy;
    };
    const dispose = () => { entries = new WeakMap(); };
    return {reactive: reactive, track: track, notify: notify, dispose: dispose};
};
