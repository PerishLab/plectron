import type { Scope, Wrap } from "@plectron/reactive";

export interface Node {
    appendChild: (node: Node) => unknown;
    removeChild: (node: Node) => unknown;
    setAttribute: (name: string, value: string) => unknown;
    setText: (text: string) => unknown;
    getValue: () => string;
    setValue: (text: string) => unknown;
    focus: () => unknown;
    addEventListener: (name: string, callback: () => unknown, options: {scope: {dispose: () => void}}) => unknown;
}

export interface Document {
    createElement: (tag: string) => Node;
    attachEditor: (host: Node, value: Node, caret: Node) => unknown;
}

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
