import { Scope } from "@plectron/reactive";

export const scope = new Scope();
export const state = scope.reactive({value: 1});
export const first = [];
export const second = [];
export const siblings = [];
scope.effect(() => {
    const value = state.value;
    first[first.length] = value;
    if (value === 2) { throw new Error("first update"); }
});
scope.effect(() => {
    const value = state.value;
    second[second.length] = value;
    if (value === 2) { throw new Error("second update"); }
});
scope.effect(() => { siblings[siblings.length] = state.value; });
export const update = (value) => { state.value = value; };

export const source = scope.reactive({value: 1});
export const values = [];
export const derived = scope.computed(() => {
    const value = source.value;
    if (value === 2) { throw new Error("computed update"); }
    return value * 2;
});
scope.effect(() => { values[values.length] = derived.value; });
export const derive = (value) => { source.value = value; };

export const loop = scope.reactive({enabled: false, left: 0, right: 0});
scope.effect(() => {
    const left = loop.left;
    if (loop.enabled) { loop.right = left + 1; }
});
scope.effect(() => {
    const right = loop.right;
    if (loop.enabled) { loop.left = right + 1; }
});
export const recurse = (enabled) => { loop.enabled = enabled; };
