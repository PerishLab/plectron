import { events, scroll } from "@plectron/engine";
import type { Bookmark, Scroll as Host, State } from "./model.ts";
export type { Bookmark, State } from "./model.ts";

type Identity<N, K> = (node: N) => K | null | undefined;
type Entry<N, K> = {key: K; node: N};

const unique = <N, K>(entries: Entry<N, K>[], key: K) => {
    for (const entry of entries) {
        if (entry.key === key) { throw new Error("duplicate scroll identity"); }
    }
};

const entries = <N, K>(host: Host<N>, identity?: Identity<N, K>) => {
    const result: Entry<N, K>[] = [];
    if (identity === undefined) { return result; }
    for (const node of host.children()) {
        const key = identity(node);
        if (key === null || key === undefined) { continue; }
        unique(result, key);
        result[result.length] = {key: key, node: node};
    }
    return result;
};

const fallbacks = <N, K>(state: State<N>, identity: Identity<N, K>) => {
    const result: {key: K; inset: number}[] = [];
    for (const fallback of state.position.fallbacks) {
        const key = identity(fallback.target);
        if (key === null || key === undefined) { continue; }
        result[result.length] = {key: key, inset: fallback.inset};
    }
    return result;
};

const bookmark = <N, K>(host: Host<N>, state: State<N> | null, identity?: Identity<N, K>): Bookmark<K> | null => {
    if (state === null) { return null; }
    if (state.edge === "end") { return {edge: "end"}; }
    if (state.position === null) { return null; }
    for (const entry of entries(host, identity)) {
        if (entry.node === state.position.target) {
            return {edge: "anchor", key: entry.key, inset: state.position.inset, fallbacks: fallbacks(state, identity)};
        }
    }
    return null;
};

const find = <N, K>(items: Entry<N, K>[], key: K) => {
    for (const entry of items) {
        if (entry.key === key) { return entry.node; }
    }
    return null;
};

const candidate = <N, K>(items: Entry<N, K>[], anchors: {key: K; inset: number}[]) => {
    for (const anchor of anchors) {
        const node = find(items, anchor.key);
        if (node !== null) { return {node: node, inset: anchor.inset}; }
    }
    return null;
};

export default function Scroll<N, K, H>(host: H & Host<N>, update?: (state: State<N>) => unknown, identity?: Identity<N, K>) {
    const controller = scroll.attach(host);
    const subscription = events.scope();
    let state: State<N> | null = null;
    let disposed = false;
    host.addEventListener("scroll", (event) => {
        state = event.scroll;
        if (update !== undefined) { update(state); }
    }, {scope: subscription});
    const restore = (value: Bookmark<K> | null) => {
        if (value === null) { return false; }
        if (value.edge === "end") {
            controller.followEnd();
            return true;
        }
        const anchors = [{key: value.key, inset: value.inset}];
        for (const fallback of value.fallbacks) { anchors[anchors.length] = fallback; }
        const found = candidate(entries(host, identity), anchors);
        if (found === null) { return false; }
        controller.restore(found.node, found.inset);
        return true;
    };
    return {
        host: host,
        followEnd: controller.followEnd,
        state: () => state,
        bookmark: () => bookmark(host, state, identity),
        restore: restore,
        dispose: () => {
            if (disposed) { return; }
            disposed = true;
            subscription.dispose();
            controller.dispose();
        }
    };
};
