import { visual } from "@plectron/engine";

export default function Visual(host, source) {
    if (source === undefined || typeof source.count !== "function" || typeof source.mount !== "function") {
        throw new Error("visual source requires count and mount");
    }
    let batch = 16;
    if (source.batch !== undefined) { batch = source.batch; }
    let margin = 0;
    if (source.margin !== undefined) { margin = source.margin; }
    const scrolling = source.scroll;
    const integer = (value) => typeof value === "number" && value % 1 === 0;
    const minimum = (left, right) => {
        if (left < right) { return left; }
        return right;
    };
    const maximum = (left, right) => {
        if (left > right) { return left; }
        return right;
    };
    if (!integer(batch) || batch < 1) { throw new Error("visual batch must be positive"); }
    if (!integer(margin) || margin < 0) { throw new Error("visual margin must be nonnegative"); }
    let rows = [];
    let start = 0;
    let end = 0;
    let previous = null;
    let expansion = null;
    let ignored = false;
    let disposed = false;
    let anchoring = null;

    const count = () => {
        const value = source.count();
        if (!integer(value) || value < 0) { throw new Error("visual count must be nonnegative"); }
        return value;
    };
    const key = (index) => {
        if (source.key === undefined) { return index; }
        return source.key(index);
    };
    const create = (index) => {
        const item = source.mount(index);
        if (item === null || item === undefined || item.host === undefined) {
            throw new Error("visual mount must return a component");
        }
        const identity = key(index);
        if (identity === null || identity === undefined) { throw new Error("visual key is absent"); }
        for (const row of rows) {
            if (row.key === identity) { throw new Error("duplicate visual key"); }
        }
        return {index: index, key: identity, item: item};
    };
    const append = (from, to) => {
        const fill = (index) => {
            if (index >= to) { return; }
            const row = create(index);
            host.appendChild(row.item.host);
            rows[rows.length] = row;
            fill(index + 1);
        };
        fill(from);
        end = to;
    };
    const prepend = (from) => {
        const staged = [];
        const fill = (index) => {
            if (index >= start) { return; }
            const row = create(index);
            staged[staged.length] = row;
            host.appendChild(row.item.host);
            fill(index + 1);
        };
        fill(from);
        for (const row of rows) { host.appendChild(row.item.host); }
        const previousRows = rows;
        rows = staged;
        for (const row of previousRows) { rows[rows.length] = row; }
        start = from;
    };
    const retire = (row) => {
        host.removeChild(row.item.host);
        if (row.item.dispose !== undefined) { row.item.dispose(); }
    };
    const preserve = (state, mutation) => {
        let bookmark = anchoring;
        if (bookmark === null && scrolling !== undefined && state.start < rows.length) {
            bookmark = {edge: "anchor", key: rows[state.start].key, inset: state.inset, fallbacks: []};
        }
        mutation();
        if (bookmark !== null && scrolling !== undefined && !scrolling.restore(bookmark)) {
            throw new Error("visual scroll anchor is absent");
        }
    };
    const reset = (anchor) => {
        for (const row of rows) { retire(row); }
        rows = [];
        previous = null;
        expansion = null;
        ignored = false;
        anchoring = null;
        const total = count();
        if (total === 0) {
            start = 0;
            end = 0;
            return;
        }
        if (anchor === undefined) {
            anchor = "start";
            if (source.anchor !== undefined) { anchor = source.anchor; }
        }
        if (anchor === "start") {
            start = 0;
            end = minimum(total, batch);
        } else if (anchor === "end") {
            end = total;
            start = maximum(0, end - batch);
        } else if (integer(anchor) && anchor >= 0 && anchor < total) {
            const half = batch / 2 - (batch / 2) % 1;
            start = maximum(0, anchor - half);
            end = minimum(total, start + batch);
            start = maximum(0, end - batch);
        } else { throw new Error("invalid visual anchor"); }
        append(start, end);
        if (scrolling !== undefined) {
            if (anchor === "end") {
                anchoring = {edge: "end"};
            } else {
                let index = start;
                if (integer(anchor)) { index = anchor; }
                anchoring = {edge: "anchor", key: key(index), inset: 0, fallbacks: []};
            }
            if (!scrolling.restore(anchoring)) { throw new Error("visual reset anchor is absent"); }
        }
    };
    const observe = (state) => {
        if (disposed) { return; }
        if (ignored) {
            ignored = false;
            previous = state;
            return;
        }
        if (state.items !== rows.length) { throw new Error("visual flow contains unmanaged children"); }
        if (expansion !== null && previous !== null) {
            const leading = expansion.edge === "leading" && state.leading <= expansion.coverage;
            if (leading && scrolling !== undefined && state.visual === expansion.visual) {
                previous = state;
                return;
            }
            if (leading) {
                throw new Error("visual flow made no leading progress");
            }
            if (expansion.edge === "trailing" && state.trailing <= expansion.coverage) {
                throw new Error("visual flow made no trailing progress");
            }
        }
        expansion = null;
        const total = count();
        const initial = previous === null;
        previous = state;
        if (state.leading < margin && start > 0 && (!initial || state.start > 0)) {
            const next = maximum(0, start - batch);
            expansion = {edge: "leading", coverage: state.leading, visual: state.visual};
            preserve(state, () => prepend(next));
            return;
        }
        if (state.trailing < margin && end < total) {
            const next = minimum(total, end + batch);
            expansion = {edge: "trailing", coverage: state.trailing, visual: state.visual};
            preserve(state, () => append(end, next));
            return;
        }
        let keepStart = 0;
        if (state.start >= batch * 2) { keepStart = batch; }
        let keepEnd = rows.length;
        if (rows.length - state.end >= batch * 2) { keepEnd = rows.length - batch; }
        if (keepStart === 0 && keepEnd === rows.length) {
            const leading = start === 0 || state.leading >= margin;
            const trailing = end === total || state.trailing >= margin;
            if (leading && trailing) { anchoring = null; }
            return;
        }
        const kept = [];
        preserve(state, () => {
            let index = 0;
            for (const row of rows) {
                if (index < keepStart || index >= keepEnd) { retire(row); }
                else { kept[kept.length] = row; }
                index = index + 1;
            }
        });
        start = start + keepStart;
        end = start + kept.length;
        rows = kept;
        ignored = true;
    };

    reset();
    const observer = visual.observe(host, observe, margin);
    return {
        host: host,
        reset: reset,
        range: () => ({start: start, end: end, total: count()}),
        item: (index) => {
            for (const row of rows) { if (row.index === index) { return row.item; } }
            return null;
        },
        dispose: () => {
            if (disposed) { return; }
            disposed = true;
            observer.dispose();
            for (const row of rows) { retire(row); }
            rows = [];
        }
    };
};
