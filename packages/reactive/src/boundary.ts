import type { Boundary, Context, Failure, Handler, Lifetime, Report, Setup, Unit } from "./model.ts";

export const create = (context: Context, lifetime: Lifetime) => {
    const nearest = (unit: Unit): Unit | null => {
        if (unit === null || unit.handler !== undefined) { return unit; }
        return nearest(unit.parent);
    };
    const diagnose = (region: Unit, failure: Failure) => {
        if (region.report === undefined) { return; }
        try { region.report(failure); }
        catch (error) { Promise.reject(error); }
    };
    const capture = (unit: Unit, error: unknown, climb: boolean): Unit | null => {
        const region = nearest(unit);
        if (region === null) { return null; }
        if (region.failed) { return region; }
        region.failed = true;
        const handler = region.handler;
        const source = region.parent;
        const control = region.control;
        const failure: Failure = {
            primary: error, secondary: undefined,
            phase: "content", generation: region.generation,
            owner: control
        };
        control.failed = true;
        control.value = undefined;
        lifetime.stop(region);
        const listener = context.active;
        const parent = context.owner;
        context.active = null;
        context.owner = source;
        try {
            handler(error, control.reset, failure);
            diagnose(region, failure);
        }
        catch (next) {
            diagnose(region, {
                primary: error, secondary: next,
                phase: "failed", generation: region.generation,
                owner: control
            });
            if (!climb) { throw next; }
            const caught = capture(source, next, true);
            if (caught === null) { throw next; }
            return caught;
        } finally {
            context.active = listener;
            context.owner = parent;
        }
        return region;
    };
    const propagate = (unit: Unit, error: unknown, climb: boolean) => {
        if (capture(unit, error, climb) === null) { throw error; }
    };
    const observe = <T>(region: Unit, version: number, value: T): T => {
        if (value === null || typeof value !== "object") { return value; }
        const pending = value as {catch?: (fn: (error: unknown) => void) => unknown};
        if (typeof pending.catch !== "function") { return value; }
        pending.catch((error) => {
            if (!region.live || region.version !== version) { throw error; }
            const caught = capture(region, error, true);
            if (caught === null) { throw error; }
        });
        return value;
    };
    const bind = <A extends unknown[], T>(region: Unit, fn: (...args: A) => T) => {
        const version = region.version;
        return (...args: A): T | undefined => {
            if (context.disposed || !region.live || region.version !== version) { return; }
            const listener = context.active;
            const parent = context.owner;
            context.active = null;
            context.owner = region;
            let value: T | undefined = undefined;
            try { value = fn(...args); }
            catch (error) {
                propagate(region, error, true);
            } finally {
                context.active = listener;
                context.owner = parent;
            }
            try { return observe(region, version, value); }
            catch (error) {
                propagate(region, error, true);
            }
        };
    };
    const boundary = <T>(fn: Setup<T>, handler: Handler, report?: Report): Boundary<T> => {
        if (context.disposed) { throw new Error("reactive scope is disposed"); }
        const anchor = context.owner;
        const epoch = anchor.epoch;
        let setup = fn;
        let fallback = handler;
        let observer = report;
        const control: Boundary<T> = {
            value: undefined, failed: false, generation: 0,
            reset: null, dispose: null
        };
        let region: Unit | null = null;
        let retired = false;
        const available = () => { return !retired && !context.disposed && anchor.live; };
        const launch = (climb: boolean) => {
            if (!available()) { return false; }
            region = {
                children: [], parent: null, live: true, failed: false,
                handler: fallback, report: observer, version: control.generation,
                generation: control.generation, control: control, epoch: 0
            };
            lifetime.adopt(region, anchor);
            control.failed = false;
            control.value = undefined;
            const parent = context.owner;
            context.owner = region;
            try { control.value = setup((callback) => bind(region, callback)); }
            catch (error) {
                propagate(region, error, climb);
            } finally { context.owner = parent; }
            return true;
        };
        control.reset = () => {
            if (!available() || anchor.epoch !== epoch || !control.failed) { return false; }
            if (region !== null && region.live) { lifetime.stop(region); }
            control.generation += 1;
            return launch(true);
        };
        control.dispose = () => {
            if (retired) { return; }
            retired = true;
            if (region !== null) { lifetime.stop(region); }
            control.value = undefined;
            region = null;
            setup = null;
            fallback = null;
            observer = null;
        };
        launch(false);
        return control;
    };
    return {boundary: boundary, capture: capture};
};
