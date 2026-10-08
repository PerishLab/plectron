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

const scenario = {
    seed: (ids) => {
        for (const row of rows) { root.removeChild(row.node); }
        rows = [];
        scenario.run("append", ids);
    },
    run: (action, ids) => {
        if (action === "append") {
            for (const id of ids) { mount(id); }
        } else if (action === "edit") {
            for (const index of ids) { rows[index].label.setText("Changed Record " + index); }
        } else if (action === "reorder") {
            if (rows.length === 0) { return; }
            const first = rows[0];
            const next = [];
            for (const row of rows) { if (row !== first) { next[next.length] = row; } }
            next[next.length] = first;
            rows = next;
            root.appendChild(first.node);
        } else if (action === "remove") {
            if (rows.length === 0) { return; }
            const first = rows[0];
            root.removeChild(first.node);
            const next = [];
            for (const row of rows) { if (row !== first) { next[next.length] = row; } }
            rows = next;
        } else { throw new Error("Unknown convergence action"); }
        status.setText("Records: " + rows.length);
    }
};
