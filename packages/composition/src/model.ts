import type { Boundary, Failure, Wrap } from "@plectron/reactive";

export interface Lifetime {
    dispose: () => void;
}

export interface Node {
    appendChild: (child: Node) => unknown;
    removeChild: (child: Node) => unknown;
    setAttribute: (name: string, value: string) => unknown;
    setText: (value: string) => unknown;
}

export interface Document<N> {
    createElement: (tag: string) => N;
}

export interface Component<N> {
    host: N;
    dispose?: () => void;
}

export interface Report {
    primary: unknown;
    secondary: unknown;
    terminal?: unknown;
    phase: "content" | "failed" | "terminal";
    generation: number;
    owner: Boundary<unknown>;
}

export interface Content<N> {
    document: Document<N>;
    wrap: Wrap;
}

export interface Recovery<N> {
    document: Document<N>;
    error: unknown;
    reset: () => boolean;
    generation: number;
}

export interface Region<N> {
    document: Document<N>;
    host: Node;
    scope: {
        boundary: <T>(setup: (wrap: Wrap) => T, handler: (error: unknown, reset: () => boolean, failure: Failure) => unknown) => Boundary<T>;
    };
    content: (context: Content<N>) => Component<N>;
    failed?: (context: Recovery<N>) => Component<N>;
    report?: (failure: Report) => unknown;
}

export interface Host<N> {
    appendChild: (child: N) => unknown;
    removeChild: (child: N) => unknown;
}

export interface Position<N> {
    target: N;
    inset: number;
    fallbacks: {target: N; inset: number}[];
}

export interface State<N> {
    previous: number;
    offset: number;
    range: number;
    edge: "start" | "middle" | "end";
    cause: "user" | "program" | "policy" | "restore" | "anchor" | "layout";
    causes: State<N>["cause"][];
    revision: number;
    position: Position<N> | null;
}

export interface Scroll<N> {
    children: () => N[];
    addEventListener: (kind: "scroll", callback: (event: {scroll: State<N>}) => unknown, options: {scope: Lifetime}) => unknown;
}

export type Bookmark<K> = {edge: "end"} | {
    edge: "anchor";
    key: K;
    inset: number;
    fallbacks: {key: K; inset: number}[];
};

export interface Observation {
    revision: number;
    leading: number;
    trailing: number;
    items: number;
    start: number;
    end: number;
    inset: number;
    visual: number;
}

export type Anchor = "start" | "end" | number;

export interface Source<N, K, C extends Component<N>> {
    count: () => number;
    mount: (index: number) => C;
    key?: (index: number) => K;
    batch?: number;
    margin?: number;
    anchor?: Anchor;
    scroll?: {restore: (bookmark: Bookmark<K | number>) => boolean};
}

export interface Row<K, C> {
    index: number;
    key: K | number;
    item: C;
}

export interface Expansion {
    edge: "leading" | "trailing";
    coverage: number;
    visual: number | undefined;
}

export interface Editor<N> {
    createElement: (tag: string) => N;
    createTextNode: (text: string) => N;
    attachEditor: (host: N, value: N, caret: N) => unknown;
}

export interface Template<N> {
    host: N;
    refs: {value: N; caret: N};
}
