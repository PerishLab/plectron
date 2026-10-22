import { document } from "@plectron/engine";

const root = document.getElementById("records");
const status = document.getElementById("status");
let rows = [];

const mount = (id) => {
    const node = document.createElement("div");
    node.setAttribute("id", "convergence-record-" + id);
    node.setAttribute("class", "record");
    const label = document.createTextNode("Record " + id);
    node.appendChild(label);
    let parent = node;
    for (const depth of [0, 1, 2]) {
        const layer = document.createElement("div");
        layer.setAttribute("class", "layer layer" + depth);
        parent.appendChild(layer);
        parent = layer;
    }
    const detail = document.createElement("span");
    detail.setAttribute("class", "detail");
    detail.setText("Resolved geometry");
    parent.appendChild(detail);
    root.appendChild(node);
    rows[rows.length] = {node: node, label: label};
};

const actions = {
    append: (ids) => {
        for (const id of ids) { mount(id); }
    },
    edit: (ids) => {
        for (const index of ids) { rows[index].label.setText("Changed Record " + index); }
    },
    reorder: () => {
        if (rows.length === 0) { return false; }
        const first = rows[0];
        const next = [];
        for (const row of rows) { if (row !== first) { next[next.length] = row; } }
        next[next.length] = first;
        rows = next;
        root.appendChild(first.node);
    },
    remove: () => {
        if (rows.length === 0) { return false; }
        const first = rows[0];
        root.removeChild(first.node);
        const next = [];
        for (const row of rows) { if (row !== first) { next[next.length] = row; } }
        rows = next;
    }
};
const scenario = {
    seed: (ids) => {
        for (const row of rows) { root.removeChild(row.node); }
        rows = [];
        scenario.run("append", ids);
    },
    run: (action, ids) => {
        const step = actions[action];
        if (step === undefined) { throw new Error("Unknown convergence action"); }
        if (step(ids) === false) { return; }
        status.setText("Records: " + rows.length);
    }
};
