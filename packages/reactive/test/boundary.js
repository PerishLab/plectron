import { Scope } from "@plectron/reactive";

export const scope = new Scope();
export const state = scope.reactive({value: 1});
export const inside = [];
export const sibling = [];
export const errors = [];
scope.boundary(() => {
    scope.effect(() => {
        const value = state.value;
        inside[inside.length] = value;
        if (value === 2) { throw new Error("protected update"); }
    });
}, (error) => { errors[errors.length] = error.message; });
scope.effect(() => { sibling[sibling.length] = state.value; });
export const update = (value) => { state.value = value; };

export const nested = [];
scope.boundary(() => {
    scope.boundary(() => {
        throw new Error("source setup");
    }, () => { throw new Error("handler setup"); });
}, (error) => { nested[nested.length] = error.message; });
export const nestedstate = scope.reactive({value: 1});
scope.boundary(() => {
    scope.boundary(() => {
        scope.effect(() => {
            if (nestedstate.value === 2) { throw new Error("nested source update"); }
        });
    }, () => { throw new Error("nested handler update"); });
}, (error) => { nested[nested.length] = error.message; });
export const updatenested = () => { nestedstate.value = 2; };

export const broken = scope.reactive({value: 1});
scope.boundary(() => {
    scope.effect(() => {
        const value = broken.value;
        if (value === 2) { throw new Error("source update"); }
    });
}, () => { throw new Error("handler update"); });
export const breakhandler = () => { broken.value = 2; };

export const wrapped = [];
let sync = null;
export const syncboundary = scope.boundary((wrap) => {
    sync = wrap((value) => {
        wrapped[wrapped.length] = value;
        return value * 2;
    });
}, () => {});
export const callsync = (value) => sync(value);
export const stopsync = () => { syncboundary.dispose(); };

export const asyncerrors = [];
let asyncjob = null;
scope.boundary((wrap) => {
    asyncjob = wrap(async () => {
        await Promise.resolve();
        throw new Error("wrapped async update");
    });
}, (error) => { asyncerrors[asyncerrors.length] = error.message; });
export const callasync = () => asyncjob();

let stalejob = null;
let rejectstale = null;
export const staleboundary = scope.boundary((wrap) => {
    stalejob = wrap(() => new Promise((resolve, reject) => {
        rejectstale = reject;
    }));
}, () => {});
export const startstale = () => stalejob();
export const stopstale = () => { staleboundary.dispose(); };
export const failstale = () => { rejectstale(new Error("stale async update")); };

export const recoverstate = scope.reactive({value: 1, broken: false});
export const recovervalues = [];
export const recoveries = [];
let retry = null;
let setups = 0;
export const recovery = scope.boundary(() => {
    setups += 1;
    scope.effect(() => {
        const value = recoverstate.value;
        recovervalues[recovervalues.length] = value;
        if (recoverstate.broken) { throw new Error("recoverable update"); }
    });
    return setups;
}, (error, reset) => { retry = reset; }, (failure) => {
    recoveries[recoveries.length] = failure.phase + ":" + failure.primary.message;
});
export const failrecovery = () => { recoverstate.broken = true; };
export const resetrecovery = () => {
    recoverstate.broken = false;
    return retry();
};

export const fallbackfailures = [];
scope.boundary(() => {
    scope.boundary(() => { throw new Error("fallback primary"); }, () => {
        throw new Error("fallback secondary");
    }, (failure) => {
        fallbackfailures[fallbackfailures.length] = failure.primary.message;
        fallbackfailures[fallbackfailures.length] = failure.secondary.message;
        fallbackfailures[fallbackfailures.length] = failure.phase;
    });
}, () => {});
