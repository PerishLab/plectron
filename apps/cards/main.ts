import { document } from "@plectron/engine";
const root = document.getElementById("cards");
const status = document.getElementById("status");
let rows = [];
let sequence = 0;
let alternate = false;
const actions = {
    append: (ids) => {
        for (const id of ids) { scenario.mount(id); }
    },
    edit: (ids) => {
        for (const index of ids) { rows[index].label.setText("Changed Card " + index); }
    },
    reorder: () => {
        if (rows.length === 0) { return false; }
        const first = rows[0];
        const rest = [];
        for (const row of rows) { if (row !== first) { rest[rest.length] = row; } }
        rest[rest.length] = first;
        rows = rest;
        root.appendChild(first.node);
    },
    remove: () => {
        if (rows.length === 0) { return false; }
        const first = rows[0];
        root.removeChild(first.node);
        const rest = [];
        for (const row of rows) { if (row !== first) { rest[rest.length] = row; } }
        rows = rest;
    },
    width: () => {
        alternate = !alternate;
        let style = "width: auto;";
        if (alternate) { style = "width: 440px;"; }
        root.setAttribute("style", style);
    }
};
const scenario = {
    seed: (ids) => {
        for (const row of rows) { root.removeChild(row.node); }
        rows = [];
        scenario.run("append", ids);
    },
    mount: (id) => {
        const node = document.createElement("div");
        node.setAttribute("id", "item-" + id);
        node.setAttribute("class", "record");
        const label = document.createTextNode("Card " + id);
        node.appendChild(label);
        const pair = document.createElement("div");
        pair.setAttribute("class", "pair");
        const primary = document.createElement("p");
        primary.setAttribute("class", "primary");
        primary.setText("A flexible workspace keeps the detail readable as its container changes.");
        const secondary = document.createElement("p");
        secondary.setAttribute("class", "secondary");
        secondary.setText("Notes and context stay together.");
        pair.appendChild(primary);
        pair.appendChild(secondary);
        node.appendChild(pair);
        root.appendChild(node);
        rows[rows.length] = {node: node, label: label};
        sequence = id + 1;
    },
    run: (action, ids) => {
        const step = actions[action];
        if (step === undefined) { throw new Error("Unknown cards action"); }
        if (step(ids) === false) { return; }
        status.setText("Records: " + rows.length);
    }
};
scenario.seed([0, 1, 2, 3, 4, 5]);
for (const action of ["append", "edit", "width"]) {
    document.getElementById(action).addEventListener("click", () => {
        let index = sequence;
        if (action === "edit") { index = 0; }
        scenario.run(action, [index]);
    });
}
