export interface Boundary<T> {
    value: T | undefined;
    failed: boolean;
    generation: number;
    reset: () => boolean;
    dispose: () => void;
}

export interface Failure {
    primary: unknown;
    secondary: unknown;
    phase: "content" | "failed";
    generation: number;
    owner: Boundary<unknown>;
}

export interface Wrap {
    <A extends unknown[], T>(callback: (...args: A) => T): (...args: A) => T | undefined;
}

export type Setup<T> = (wrap: Wrap) => T;
export type Handler = (error: unknown, reset: () => boolean, failure: Failure) => unknown;
export type Report = (failure: Failure) => unknown;
export type Kind = "get" | "has" | "keys" | "iterate";

export interface Dependency {
    kind: Kind;
    key: PropertyKey | undefined;
    units: Unit[];
}

export interface Entry {
    deps: Dependency[];
}

export interface Proxy extends Entry {
    raw: object;
    proxy: object | null;
    writing: number;
    array: boolean;
    method: Function | null;
    iterate: Function | null;
}

export interface Mutation {
    key: PropertyKey;
    before: PropertyDescriptor | undefined;
    after: PropertyDescriptor | undefined;
    existed: boolean;
    exists: boolean;
    previous: number;
    length: number;
}

export interface Unit {
    children: Unit[];
    parent: Unit | null;
    live: boolean;
    epoch: number;
    deps?: Dependency[];
    fn?: () => unknown;
    computed?: boolean;
    entry?: Entry;
    ready?: boolean;
    dirty?: boolean;
    running?: boolean;
    value?: unknown;
    handler?: Handler;
    report?: Report;
    version?: number;
    failed?: boolean;
    generation?: number;
    control?: Boundary<unknown>;
}

export interface Scheduler {
    enqueue: (unit: Unit) => void;
    cancel: (unit: Unit) => void;
    batch: <T>(fn: () => T) => T;
    dispose: () => void;
}

export interface Context {
    active: Unit | null;
    owner: Unit;
    disposed: boolean;
    scheduler: Scheduler | null;
}

export interface Lifetime {
    stop: (unit: Unit) => void;
    adopt: (unit: Unit, parent: Unit) => void;
}
