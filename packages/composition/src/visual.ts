import { visual } from "@plectron/engine";
import type { Anchor, Bookmark, Component, Expansion, Host, Observation, Row, Source } from "./model.ts";
export type { Anchor, Observation, Source } from "./model.ts";
import { bookmark, integer, item, maximum, minimum, select } from "./range.ts";

export default function Visual<N, K, C extends Component<N>>(host: Host<N>, source: Source<N, K, C>) {
    if (source === undefined || typeof source.count !== "function" || typeof source.mount !== "function") {
        throw new Error("visual source requires count and mount");
    }
    let batch = 16;
    if (source.batch !== undefined) { batch = source.batch; }
    let margin = 0;
    if (source.margin !== undefined) { margin = source.margin; }
    const scrolling = source.scroll;
    if (!integer(batch) || batch < 1) { throw new Error("visual batch must be positive"); }
    if (!integer(margin) || margin < 0) { throw new Error("visual margin must be nonnegative"); }
    let rows: Row<K, C>[] = [];
    let start = 0;
    let end = 0;
    let previous: Observation | null = null;
    let expansion: Expansion | null = null;
    let ignored = false;
    let disposed = false;
    let anchoring: Bookmark<K | number> | null = null;

    const count = () => {
        const value = source.count();
        if (!integer(value) || value < 0) { throw new Error("visual count must be nonnegative"); }
        return value;
    };
    const key = (index: number) => {
        if (source.key === undefined) { return index; }
        return source.key(index);
    };
    const create = (index: number) => {
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
    const append = (from: number, to: number) => {
        const fill = (index: number) => {
            if (index >= to) { return; }
            const row = create(index);
            host.appendChild(row.item.host);
            rows[rows.length] = row;
            fill(index + 1);
        };
        fill(from);
        end = to;
    };
    const prepend = (from: number) => {
        const staged: Row<K, C>[] = [];
        const fill = (index: number) => {
            if (index >= start) { return; }
            const row = create(index);
            staged[staged.length] = row;
            host.appendChild(row.item.host);
            fill(index + 1);
        };
        fill(from);
        for (const row of rows) { host.appendChild(row.item.host); }
        const retained = rows;
        rows = staged;
        for (const row of retained) { rows[rows.length] = row; }
        start = from;
    };
    const retire = (row: Row<K, C>) => {
        host.removeChild(row.item.host);
        if (row.item.dispose !== undefined) { row.item.dispose(); }
    };
    const preserve = (state: Observation, mutation: () => void) => {
        let bookmark = anchoring;
        if (bookmark === null && scrolling !== undefined && state.start < rows.length) {
            bookmark = {edge: "anchor", key: rows[state.start].key, inset: state.inset, fallbacks: []};
        }
        mutation();
        if (bookmark !== null && scrolling !== undefined && !scrolling.restore(bookmark)) {
            throw new Error("visual scroll anchor is absent");
        }
    };
    const reset = (anchor?: Anchor) => {
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
        const selected = select(anchor, total, batch);
        start = selected.start;
        end = selected.end;
        append(start, end);
        if (scrolling === undefined) { return; }
        anchoring = bookmark(anchor, start, key);
        if (!scrolling.restore(anchoring)) { throw new Error("visual reset anchor is absent"); }
    };
    const prune = (first: number, last: number) => {
        const kept: Row<K, C>[] = [];
        let index = 0;
        for (const row of rows) {
            if (index < first || index >= last) { retire(row); }
            else { kept[kept.length] = row; }
            index = index + 1;
        }
        return kept;
    };
    const observe = (state: Observation) => {
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
        let leading = state.start;
        if (previous !== null) { leading = 1; }
        previous = state;
        if (state.leading < margin && start > 0 && leading > 0) {
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
        let first = 0;
        if (state.start >= batch * 2) { first = batch; }
        let last = rows.length;
        if (rows.length - state.end >= batch * 2) { last = rows.length - batch; }
        if (first === 0 && last === rows.length) {
            const leading = start === 0 || state.leading >= margin;
            const trailing = end === total || state.trailing >= margin;
            if (leading && trailing) { anchoring = null; }
            return;
        }
        let kept: Row<K, C>[] = [];
        preserve(state, () => { kept = prune(first, last); });
        start = start + first;
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
        item: (index: number) => item(rows, index),
        dispose: () => {
            if (disposed) { return; }
            disposed = true;
            observer.dispose();
            for (const row of rows) { retire(row); }
            rows = [];
        }
    };
};
