import { document, events, bridge, shortcuts } from "@plectron/engine";
type Item = {id: number; title: string; done: boolean; node?: ReturnType<typeof document.createElement>; scope?: ReturnType<typeof events.scope>};
let items: Item[] = [
    {id: 0, title: "Model boundaries", done: false},
    {id: 1, title: "Check semantics", done: false},
    {id: 2, title: "Render changes", done: false}
];
let sequence = items.length;
const list = document.getElementById("list");
const status = document.getElementById("status");
const title = document.getElementById("title");
title.focus();
const bindings = shortcuts.register("editor", 0, [
    {chord: "Primary+Enter", command: "commit"},
    {chord: "Primary+KeyK", command: "selectAll"}
]);

const update = (item: Item) => {
    if (item.done) {
        item.node.setText("Done: " + item.title);
        item.node.setAttribute("style", "background-color: #88c9a5;");
    } else {
        item.node.setText("Todo: " + item.title);
        item.node.setAttribute("style", "background-color: #d8e2f0;");
    }
};

const mount = (...batch: Item[]) => {
    for (const item of batch) {
        const node = document.createElement("button");
        node.setAttribute("id", "task-" + item.id);
        node.setAttribute("class", "task");
        item.node = node;
        item.scope = events.scope();
        node.addEventListener("click", (event) => {
            item.done = !item.done;
            update(item);
            status.setText("Changed task " + item.id);
            bridge.postMessage("changed:" + item.id);
        }, {scope: item.scope});
        update(item);
        list.appendChild(node);
    }
};

mount(...items);

const add = () => {
    const name = title.getValue();
    const item = {id: sequence, title: name || "Task " + sequence, done: false};
    sequence = sequence + 1;
    items = [...items, item];
    mount(item);
    status.setText("Tasks: " + items.length);
    bridge.postMessage("added:" + item.id);
    title.setValue("");
    title.focus();
};

document.getElementById("add").addEventListener("click", add);
title.addEventListener("commit", add);
title.addEventListener("input", () => {
    status.setText("Editing: " + title.getValue());
});

document.getElementById("remove").addEventListener("click", (event) => {
    const remaining = [];
    for (const item of items) {
        if (item.done) {
            item.scope.dispose();
            list.removeChild(item.node);
            continue;
        }
        remaining[remaining.length] = item;
    }
    items = remaining;
    status.setText("Tasks: " + items.length);
    bridge.postMessage("remaining:" + items.length);
});

bridge.onMessage((message) => {
    if (message === "reset") {
        for (const item of items) {
            item.done = false;
            update(item);
        }
        status.setText("Reset");
    }
});
