const createArray = function (track, notify, changed, unwrap) {
    const position = (key) => {
        if (typeof key !== "string") { return -1; }
        const value = +key;
        if (value < 0 || value >= 4294967295 || value % 1 !== 0) { return -1; }
        if ("" + value === key) { return value; }
        return -1;
    };
    const change = (entry, key, before, after, previous, length) => {
        if (!entry.array) { return; }
        if (previous !== length) {
            notify(entry, "get", "length");
            if (length < previous) {
                notify(entry, "keys", undefined);
                for (const dep of entry.deps) {
                    const index = position(dep.key);
                    if (index >= length && index < previous
                        && (dep.kind === "get" || dep.kind === "has")) {
                        notify(entry, dep.kind, dep.key);
                    }
                }
            }
        }
        if ((key === "length" || position(key) >= 0)
            && (changed(before, after) || previous !== length)) {
            notify(entry, "iterate", undefined);
        }
    };
    const length = (entry, target) => {
        if (entry.array) { return target.length; }
        return 0;
    };
    const iterate = (entry, method) => {
        return function (...args) {
            const receiver = this;
            const source = Reflect.apply(method, receiver, args);
            if (unwrap(receiver) !== entry.raw || source === null
                || typeof source !== "object") { return source; }
            const next = Reflect.get(source, "next");
            const iterator = {
                next(...input) {
                    track(entry, "iterate", undefined);
                    return Reflect.apply(next, source, input);
                },
                [Symbol.iterator]() { return this; }
            };
            const close = Reflect.get(source, "return");
            if (close !== undefined && close !== null) {
                iterator.return = function (...input) {
                    return Reflect.apply(close, source, input);
                };
            }
            return iterator;
        };
    };
    return {change: change, length: length, iterate: iterate};
};

export { createArray };
