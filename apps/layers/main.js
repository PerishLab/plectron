import { document } from "@plectron/engine";
const root = document.getElementById("layers");
const status = document.getElementById("status");
let rows = [];
let sequence = 0;
let alternate = false;
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
        const label = document.createTextNode("Panel " + id);
        node.appendChild(label);
        const crop = document.createElement("div");
        crop.setAttribute("class", "crop");
        const plane = document.createElement("div");
        plane.setAttribute("class", "plane");
        const inset = document.createElement("p");
        inset.setAttribute("class", "inset");
        inset.setText("A shifted layer keeps its own color and clipping boundary.");
        plane.appendChild(inset);
        crop.appendChild(plane);
        node.appendChild(crop);
        root.appendChild(node);
        rows[rows.length] = {node: node, label: label};
        sequence = id + 1;
    },
    run: (action, ids) => {
        if (action === "append") {
            for (const id of ids) { scenario.mount(id); }
        } else if (action === "edit") {
            for (const index of ids) { rows[index].label.setText("Changed Panel " + index); }
        } else if (action === "reorder") {
            if (rows.length === 0) { return; }
            const first = rows[0];
            const rest = [];
            for (const row of rows) { if (row !== first) { rest[rest.length] = row; } }
            rest[rest.length] = first;
            rows = rest;
            root.appendChild(first.node);
        } else if (action === "remove") {
            if (rows.length === 0) { return; }
            const first = rows[0];
            root.removeChild(first.node);
            const rest = [];
            for (const row of rows) { if (row !== first) { rest[rest.length] = row; } }
            rows = rest;
        } else if (action === "fade") {
            alternate = !alternate;
            let style = "opacity: 1;";
            if (alternate) { style = "opacity: 0.6;"; }
            root.setAttribute("style", style);
        } else if (action === "hide") {
            root.setAttribute("style", "opacity: 0; translate: 12px 0px;");
        } else if (action === "show") {
            root.setAttribute("style", "opacity: 0.6; translate: 12px 0px;");
        } else if (action === "shift") {
            root.setAttribute("style", "opacity: 0.6; translate: 12px 0px;");
        } else { throw new Error("Unknown layers action"); }
        status.setText("Records: " + rows.length);
    }
};
scenario.seed([0, 1, 2, 3, 4, 5]);
for (const action of ["append", "edit", "fade", "shift"]) {
    document.getElementById(action).addEventListener("click", () => {
        let index = sequence;
        if (action === "edit") { index = 0; }
        scenario.run(action, [index]);
    });
}
