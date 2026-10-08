import type { Anchor, Bookmark, Row } from "./model.ts";

export const integer = (value: unknown): value is number => { return typeof value === "number" && value % 1 === 0; };

export const minimum = (left: number, right: number) => {
    if (left < right) { return left; }
    return right;
};

export const maximum = (left: number, right: number) => {
    if (left > right) { return left; }
    return right;
};

export const select = (anchor: Anchor, total: number, batch: number) => {
    if (typeof anchor === "number") {
        if (!integer(anchor) || anchor < 0 || anchor >= total) { throw new Error("invalid visual anchor"); }
        const half = batch / 2 - (batch / 2) % 1;
        let start = maximum(0, anchor - half);
        const end = minimum(total, start + batch);
        start = maximum(0, end - batch);
        return {start: start, end: end};
    }
    if (anchor !== "start" && anchor !== "end") { throw new Error("invalid visual anchor"); }
    let end = total;
    if (anchor === "start") { end = minimum(total, batch); }
    return {start: maximum(0, end - batch), end: end};
};

export const bookmark = <K>(anchor: Anchor, start: number, key: (index: number) => K): Bookmark<K> => {
    if (anchor === "end") { return {edge: "end"}; }
    let index = start;
    if (integer(anchor)) { index = anchor; }
    return {edge: "anchor", key: key(index), inset: 0, fallbacks: []};
};

export const item = <K, C>(rows: Row<K, C>[], index: number) => {
    for (const row of rows) {
        if (row.index === index) { return row.item; }
    }
    return null;
};
