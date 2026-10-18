import { Scope } from "@plectron/reactive";

export const scope = new Scope();
export const scopeholder = (() => {
    const owner = new Scope();
    const value = {scope: true};
    owner.effect(() => value);
    return {owner: owner, value: value};
})();
export const releasescope = () => {
    scopeholder.owner = null;
    scopeholder.value = null;
};
export const holder = (() => {
    const raw = {value: 1};
    const proxy = scope.reactive(raw);
    return {raw: raw, proxy: proxy};
})();
export const release = () => {
    holder.raw = null;
    holder.proxy = null;
};
export const effectholder = (() => {
    const value = {effect: true};
    const stop = scope.effect(() => value);
    return {value: value, stop: stop};
})();
export const releaseeffect = () => {
    effectholder.stop();
    effectholder.stop = null;
    effectholder.value = null;
};
export const pendingholder = (() => {
    const value = {pending: true};
    const state = scope.reactive({value: 1});
    const stop = scope.effect(() => { state.value; return value; });
    return {value: value, state: state, stop: stop};
})();
export const releasepending = () => {
    pendingholder.state.value = 2;
    pendingholder.stop();
    pendingholder.stop = null;
    pendingholder.value = null;
};
export const computedholder = (() => {
    const owner = new Scope();
    const value = {computed: true};
    const result = owner.computed(() => value);
    result.value;
    return {owner: owner, value: value, result: result};
})();
export const releasecomputed = () => {
    computedholder.owner.dispose();
    computedholder.owner = null;
    computedholder.value = null;
    computedholder.result = null;
};
export const boundaryholder = (() => {
    const value = {boundary: true};
    const state = scope.reactive({failed: false});
    scope.boundary(() => {
        scope.effect(() => {
            if (state.failed) { throw new Error("boundary collection"); }
        });
    }, () => value);
    return {value: value, state: state};
})();
export const releaseboundary = () => {
    boundaryholder.state.failed = true;
    boundaryholder.value = null;
};
