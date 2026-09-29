import { document } from "@plectron/engine";
const root = document.getElementById("gallery");
const status = document.getElementById("status");
let rows = [];
let sequence = 0;
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
        const label = document.createTextNode("View " + id);
        node.appendChild(label);
        const picture = document.createElement("img");
        picture.setAttribute("src", "../pictures/dawn.png");
        node.appendChild(picture);
        root.appendChild(node);
        rows[rows.length] = {node: node, label: label, picture: picture};
        sequence = id + 1;
    },
    run: (action, ids) => {
        if (action === "append") {
            for (const id of ids) { scenario.mount(id); }
        } else if (action === "edit") {
            for (const index of ids) { rows[index].label.setText("Changed View " + index); }
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
        } else if (action === "swap" || action === "restore") {
            let source = "../pictures/dawn.png";
            if (action === "swap") { source = "../pictures/dusk.png"; }
            for (const row of rows) { row.picture.setAttribute("src", source); }
        } else { throw new Error("Unknown gallery action"); }
        status.setText("Records: " + rows.length);
    }
};
scenario.seed([0, 1, 2, 3, 4, 5]);
for (const action of ["append", "edit", "swap", "restore"]) {
    document.getElementById(action).addEventListener("click", () => {
        let index = sequence;
        if (action === "edit") { index = 0; }
        scenario.run(action, [index]);
    });
}
