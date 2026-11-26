import type { Scope, Wrap } from "@plectron/reactive";

export type Node =
    & {focus: () => unknown}
    & Record<"appendChild" | "removeChild", (node: Node) => unknown>
    & Record<"setAttribute", (name: string, value: string) => unknown>
    & Record<"setText" | "setValue", (text: string) => unknown>
    & Record<"getValue", () => string>
    & Record<"addEventListener", (name: string, callback: () => unknown, options: {scope: {dispose: () => void}}) => unknown>;

export type Document =
    & Record<"createElement", (tag: string) => Node>
    & Record<"createTextNode", (text: string) => Node>
    & Record<"attachEditor", (host: Node, value: Node, caret: Node) => unknown>;

export interface Item {
    id: number;
    title: string;
    done: boolean;
}

export interface Context {
    document: Document;
    scope: Scope;
    wrap: Wrap;
}

export interface Row {
    document: Document;
    wrap: Wrap;
    props: {
        item: Item;
        filter: () => string;
        scope: Scope;
        status: (text: string) => unknown;
        remove: (item: Item) => unknown;
    };
}
