import { Scope } from "@plectron/reactive";

const scope = new Scope();
export const state = scope.reactive({count: 1, nested: {value: 2}});
export const identity = scope.reactive(state) === state
    && state.nested === state.nested;
export const values = [];
scope.effect(() => {
    values[values.length] = state.count + state.nested.value;
});
state.count = 2;
state.count = 3;

export const branch = scope.reactive({pick: true, left: 1, right: 5});
export const branches = [];
export const halt = scope.effect(() => {
    if (branch.pick) { branches[branches.length] = branch.left; }
    else { branches[branches.length] = branch.right; }
});
branch.pick = false;

export const list = scope.reactive([1, 2, 3]);
export const traversals = [];
export const methods = [];
scope.effect(() => {
    for (const value of list) {
        traversals[traversals.length] = value;
        break;
    }
});
scope.effect(() => { methods[methods.length] = list[Symbol.iterator]; });

export const parity = scope.computed(() => state.count % 2);
export const parities = [];
scope.effect(() => { parities[parities.length] = parity.value; });
