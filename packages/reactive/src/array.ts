import type { Entry, Kind, Mutation, Proxy } from "./model.ts";

const position = (key: PropertyKey) => {
    if (typeof key !== "string") { return -1; }
    const value = +key;
    if (value < 0 || value >= 4294967295 || value % 1 !== 0) { return -1; }
    if ("" + value === key) { return value; }
    return -1;
};

const forward = (source: object, method: Function) => {
    return function (...input: unknown[]) { return Reflect.apply(method, source, input); };
};

export const create = (
    track: (entry: Entry, kind: Kind, key: PropertyKey) => void,
    notify: (entry: Entry, kind: Kind, key: PropertyKey) => void,
    changed: (before: PropertyDescriptor, after: PropertyDescriptor) => boolean,
    unwrap: <T>(value: T) => T
) => {
    const truncate = (entry: Proxy, mutation: Mutation) => {
        notify(entry, "keys", undefined);
        for (const dep of entry.deps) {
            const index = position(dep.key);
            if (index < mutation.length || index >= mutation.previous) { continue; }
            if (dep.kind === "get" || dep.kind === "has") { notify(entry, dep.kind, dep.key); }
        }
    };
    const traversed = (mutation: Mutation) => {
        if (mutation.key !== "length" && position(mutation.key) < 0) { return false; }
        return changed(mutation.before, mutation.after) || mutation.previous !== mutation.length;
    };
    const change = (entry: Proxy, mutation: Mutation) => {
        if (!entry.array) { return; }
        const previous = mutation.previous;
        const length = mutation.length;
        if (previous !== length) {
            notify(entry, "get", "length");
            if (length < previous) {
                truncate(entry, mutation);
            }
        }
        if (traversed(mutation)) {
            notify(entry, "iterate", undefined);
        }
    };
    const length = (entry: Proxy, target: object): number => {
        if (entry.array) { return Reflect.get(target, "length"); }
        return 0;
    };
    const iterate = (entry: Proxy, method: Function) => {
        return function (this: unknown, ...args: unknown[]) {
            const receiver = this;
            const source = Reflect.apply(method, receiver, args);
            if (unwrap(receiver) !== entry.raw || source === null
                || typeof source !== "object") { return source; }
            const next = Reflect.get(source, "next");
            const iterator: {
                next: (...input: unknown[]) => unknown;
                return?: (...input: unknown[]) => unknown;
                [Symbol.iterator]: () => unknown;
            } = {
                next(...input) {
                    track(entry, "iterate", undefined);
                    return Reflect.apply(next, source, input);
                },
                [Symbol.iterator]() { return this; }
            };
            const close = Reflect.get(source, "return");
            if (close !== undefined && close !== null) {
                iterator.return = forward(source, close);
            }
            return iterator;
        };
    };
    return {change: change, length: length, iterate: iterate};
};
