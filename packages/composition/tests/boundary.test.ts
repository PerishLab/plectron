import test from "node:test";
import assert from "node:assert/strict";
import Boundary from "@plectron/composition/boundary";
import { Scope } from "@plectron/reactive";

import type { Report } from "@plectron/composition/boundary";

const checkpoint = () => { return Promise.resolve(); };

const environment = () => {
    const element = (tag: string) => {
        const node = {
            tag: tag, attributes: {}, children: [], text: "",
            setAttribute(name, value) { this.attributes[name] = value; },
            setText(value) { this.text = value; },
            appendChild(child) { this.children[this.children.length] = child; },
            removeChild(child) {
                const index = this.children.indexOf(child);
                if (index < 0) { throw new Error("missing child"); }
                this.children.splice(index, 1);
            }
        };
        return node;
    };
    return {document: {createElement: element}, host: element("section")};
};

test("a boundary replaces failed content and resets from zero", async () => {
    const scope = new Scope();
    const state = scope.reactive({broken: false});
    const frame = environment();
    const reports: Report[] = [];
    const retired = [];
    let retry: (() => boolean) | null = null;
    let setup = 0;
    const boundary = Boundary({
        document: frame.document,
        host: frame.host,
        scope: scope,
        content: () => {
            setup += 1;
            const node = frame.document.createElement("article");
            node.setText("content " + setup);
            scope.effect(() => {
                if (state.broken) { throw new Error("broken content"); }
            });
            return {host: node, dispose: () => { retired[retired.length] = node.text; }};
        },
        failed: ({error, reset}) => {
            retry = reset;
            const node = frame.document.createElement("aside");
            assert.ok(error instanceof Error);
            node.setText(error.message);
            return {host: node, dispose: () => { retired[retired.length] = "fallback"; }};
        },
        report: (failure) => { reports[reports.length] = failure; }
    });
    assert.equal(frame.host.children[0].text, "content 1");
    state.broken = true;
    await checkpoint();
    assert.equal(frame.host.children.length, 1);
    assert.equal(frame.host.children[0].text, "broken content");
    assert.deepEqual(retired, ["content 1"]);
    assert.equal(boundary.failed, true);
    assert.equal(reports[0].phase, "content");
    state.broken = false;
    assert.equal(retry(), true);
    assert.equal(frame.host.children[0].text, "content 2");
    assert.deepEqual(retired, ["content 1", "fallback"]);
    assert.equal(boundary.generation, 1);
    assert.equal(boundary.failed, false);
    boundary.dispose();
    assert.deepEqual(retired, ["content 1", "fallback", "content 2"]);
    scope.dispose();
});

test("a missing fallback uses the fixed terminal", () => {
    const scope = new Scope();
    const frame = environment();
    const reports: Report[] = [];
    const boundary = Boundary({
        document: frame.document,
        host: frame.host,
        scope: scope,
        content: () => { throw new Error("unavailable"); },
        report: (failure) => { reports[reports.length] = failure; }
    });
    const terminal = frame.host.children[0];
    assert.equal(terminal.tag, "div");
    assert.equal(terminal.attributes.role, "alert");
    assert.equal(terminal.attributes["data-plectron-terminal"], "");
    assert.equal(terminal.text, "Content failed");
    assert.equal(reports.length, 1);
    assert.ok(reports[0].primary instanceof Error);
    assert.equal(reports[0].primary.message, "unavailable");
    assert.equal(reports[0].secondary, undefined);
    boundary.dispose();
    scope.dispose();
});

test("a broken fallback records both errors before using the terminal", () => {
    const scope = new Scope();
    const frame = environment();
    const reports: Report[] = [];
    const boundary = Boundary({
        document: frame.document,
        host: frame.host,
        scope: scope,
        content: () => { throw new Error("primary"); },
        failed: () => { throw new Error("secondary"); },
        report: (failure) => { reports[reports.length] = failure; }
    });
    assert.equal(frame.host.children[0].text, "Content failed");
    assert.equal(reports.length, 1);
    assert.ok(reports[0].primary instanceof Error);
    assert.equal(reports[0].primary.message, "primary");
    assert.ok(reports[0].secondary instanceof Error);
    assert.equal(reports[0].secondary.message, "secondary");
    assert.equal(reports[0].phase, "failed");
    boundary.dispose();
    scope.dispose();
});
