import type { Bookmark, State } from "@plectron/composition/scroll";

export interface Node {
    appendChild: (node: Node) => unknown;
    removeChild: (node: Node) => unknown;
    children: () => Node[];
    getAttribute: (name: string) => string | null;
    setAttribute: (name: string, value: string) => unknown;
    setText: (text: string) => unknown;
    getValue: () => string;
    setValue: (text: string) => unknown;
    focus: () => unknown;
    addEventListener: (name: string, callback: (event: {scroll: State<Node>}) => unknown, options: {scope: {dispose: () => void}}) => unknown;
}

export interface Document {
    createElement: (tag: string) => Node;
    attachEditor: (host: Node, value: Node, caret: Node) => unknown;
}

export interface Message {
    id: number;
    author: string;
    avatar: string;
    body: string;
    time: string;
}

export type Position = Bookmark<string> | null;
export type Name = "general" | "engine";
