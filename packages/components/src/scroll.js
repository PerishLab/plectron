import { events, scroll } from "@plectron/engine";

export default function Scroll(host, update, identity) {
    const controller = scroll.attach(host);
    const subscription = events.scope();
    let state = null;
    let disposed = false;
    const entries = () => {
        const result = [];
        if (identity === undefined) { return result; }
        for (const node of host.children()) {
            const key = identity(node);
            if (key === null || key === undefined) { continue; }
            for (const entry of result) {
                if (entry.key === key) { throw new Error("duplicate scroll identity"); }
            }
            result[result.length] = {key: key, node: node};
        }
        return result;
    };
    host.addEventListener("scroll", (event) => {
        state = event.scroll;
        if (update !== undefined) { update(state); }
    }, {scope: subscription});
    return {
        host: host,
        followEnd: controller.followEnd,
        state: () => state,
        bookmark: () => {
            if (state === null) { return null; }
            if (state.edge === "end") { return {edge: "end"}; }
            if (state.position === null) { return null; }
            const items = entries();
            for (const entry of items) {
                if (entry.node === state.position.target) {
                    const fallbacks = [];
                    for (const fallback of state.position.fallbacks) {
                        const key = identity(fallback.target);
                        if (key !== null && key !== undefined) {
                            fallbacks[fallbacks.length] = {key: key, inset: fallback.inset};
                        }
                    }
                    return {edge: "anchor", key: entry.key, inset: state.position.inset, fallbacks: fallbacks};
                }
            }
            return null;
        },
        restore: (bookmark) => {
            if (bookmark === null) { return false; }
            if (bookmark.edge === "end") {
                controller.followEnd();
                return true;
            }
            const items = entries();
            const candidates = [{key: bookmark.key, inset: bookmark.inset}];
            for (const fallback of bookmark.fallbacks) {
                candidates[candidates.length] = fallback;
            }
            for (const candidate of candidates) {
                for (const entry of items) {
                    if (entry.key === candidate.key) {
                        controller.restore(entry.node, candidate.inset);
                        return true;
                    }
                }
            }
            return false;
        },
        dispose: () => {
            if (disposed) { return; }
            disposed = true;
            subscription.dispose();
            controller.dispose();
        }
    };
};
