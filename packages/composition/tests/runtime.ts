import Boundary from "@plectron/composition/boundary";
import type { Report } from "@plectron/composition/boundary";
import { Scope } from "@plectron/reactive";

interface Element {
    tag: string;
    children: Element[];
    attributes: Record<string, string>;
    text: string;
    appendChild: (child: Element) => void;
    removeChild: (child: Element) => void;
    setAttribute: (name: string, value: string) => void;
    setText: (value: string) => void;
}

const remove = (children: Element[], child: Element) => {
    const retained: Element[] = [];
    for (const node of children) {
        if (node !== child) { retained[retained.length] = node; }
    }
    return retained;
};

const element = (tag: string): Element => {
    return {
        tag: tag, children: [], attributes: {}, text: "",
        appendChild(child) { this.children[this.children.length] = child; },
        removeChild(child) { this.children = remove(this.children, child); },
        setAttribute(name, value) { this.attributes[name] = value; },
        setText(value) { this.text = value; }
    };
};

const document = {createElement: element};
export const host = element("section");
export const scope = new Scope();
export const state = scope.reactive({broken: false});
export const reports: Report[] = [];
export const retired: string[] = [];
let setups = 0;
export const region = Boundary({
    document: document, host: host, scope: scope,
    content: () => {
        setups += 1;
        const node = element("article");
        node.setText("content " + setups);
        scope.effect(() => {
            if (state.broken) { throw new Error("broken content"); }
        });
        return {host: node, dispose: () => { retired[retired.length] = node.text; }};
    },
    failed: (context) => {
        const error = context.error;
        if (!(error instanceof Error)) { throw error; }
        const node = element("aside");
        node.setText(error.message);
        return {host: node, dispose: () => { retired[retired.length] = "fallback"; }};
    },
    report: (failure) => { reports[reports.length] = failure; }
});
export const terminal = element("section");
export const failures: Report[] = [];
export const broken = Boundary({
    document: document, host: terminal, scope: scope,
    content: () => { throw new Error("primary"); },
    failed: () => { throw new Error("secondary"); },
    report: (failure) => { failures[failures.length] = failure; }
});
export const inert = element("section");
export const missing = Boundary({
    document: document, host: inert, scope: scope,
    content: () => { throw new Error("unavailable"); }
});
