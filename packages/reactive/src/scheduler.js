const LIMIT = 16;

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

const create = function (execute, failure) {
    const pending = [];
    const derived = [];
    let scheduled = false;
    let flushing = false;
    let settling = false;
    let depth = 0;
    const runs = [];
    const errors = [];

    const reject = (unit, error) => {
        try {
            if (!failure(unit, error)) { errors[errors.length] = error; }
        } catch (next) { errors[errors.length] = next; }
    };
    const invoke = (unit) => {
        let entry = null;
        for (const current of runs) {
            if (current.unit === unit) { entry = current; break; }
        }
        if (entry === null) {
            entry = {unit: unit, count: 0};
            runs[runs.length] = entry;
        }
        entry.count += 1;
        if (entry.count > LIMIT) {
            reject(unit, new Error("reactive update limit exceeded"));
            return;
        }
        try { execute(unit); }
        catch (error) { reject(unit, error); }
    };
    const report = (errors, index) => {
        if (index >= errors.length) { return; }
        Promise.reject(errors[index]);
        report(errors, index + 1);
    };
    const flush = () => {
        scheduled = false;
        flushing = true;
        try {
            for (const unit of derived) { invoke(unit); }
            derived.length = 0;
            const effects = [...pending];
            pending.length = 0;
            for (const unit of effects) { invoke(unit); }
        } finally { flushing = false; }
        if (derived.length > 0 || pending.length > 0) {
            schedule();
            return;
        }
        const failures = [...errors];
        runs.length = 0;
        errors.length = 0;
        settling = false;
        report(failures, 1);
        if (failures.length > 0) { throw failures[0]; }
    };
    const schedule = () => {
        if (scheduled || flushing || depth > 0) { return; }
        if (!settling) { settling = true; }
        scheduled = true;
        Promise.resolve().then(flush);
    };
    const enqueue = (unit) => {
        let queue = pending;
        if (unit.computed) { queue = derived; }
        for (const current of queue) {
            if (current === unit) { return; }
        }
        queue[queue.length] = unit;
        schedule();
    };
    const cancel = (unit) => {
        remove(pending, unit);
        remove(derived, unit);
    };
    const batch = (fn) => {
        depth += 1;
        try { return fn(); }
        finally {
            depth -= 1;
            if (depth === 0 && (pending.length > 0 || derived.length > 0)) {
                schedule();
            }
        }
    };
    const dispose = () => {
        pending.length = 0;
        derived.length = 0;
    };
    return {enqueue: enqueue, cancel: cancel, batch: batch, dispose: dispose};
};

export { create };
