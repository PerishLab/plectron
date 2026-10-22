import { document, events } from "@plectron/engine";
const left = document.getElementById("left");
const right = document.getElementById("right");
const status = document.getElementById("status");
type Row = {id: number; parent: ReturnType<typeof document.getElementById>; node: ReturnType<typeof document.createElement>; scope: ReturnType<typeof events.scope>};
let rows: Row[] = [];
let sequence = 0;
const actions = {
    append: (ids: number[]) => {
        for (const id of ids) { scenario.mount(id, left); }
    },
    edit: (ids: number[]) => {
        for (const index of ids) {
            let text = "Changed record";
            if (index !== 0) { text = text + " " + index; }
            rows[index].node.setText(text);
        }
    },
    reorder: () => {
        const first = rows[0];
        const rest = [];
        for (const row of rows) { if (row !== first) { rest[rest.length] = row; } }
        rest[rest.length] = first;
        rows = rest;
        first.parent.appendChild(first.node);
    },
    remove: () => {
        const first = rows[0];
        first.parent.removeChild(first.node);
        first.scope.dispose();
        const rest = [];
        for (const row of rows) { if (row !== first) { rest[rest.length] = row; } }
        rows = rest;
    },
    focus: () => {
        document.getElementById("right-input").focus();
    }
};
const scenario = {
    seed: (ids: number[]) => {
        for (const row of rows) { row.parent.removeChild(row.node); row.scope.dispose(); }
        rows = [];
        for (const id of ids) {
            let parent = left;
            if (id % 2 !== 0) { parent = right; }
            scenario.mount(id, parent);
        }
        status.setText("Rows: " + rows.length);
    },
    mount: (id: number, parent: Row["parent"]) => {
        const node = document.createElement("button");
        node.setAttribute("id", "item-" + id);
        node.setAttribute("class", "row");
        node.setText("Record " + id);
        const scope = events.scope();
        node.addEventListener("click", () => status.setText("Selected " + id), {scope: scope});
        parent.appendChild(node);
        rows[rows.length] = {id: id, parent: parent, node: node, scope: scope};
        sequence = id + 1;
    },
    run: (action: string, ids: number[]) => {
        if (rows.length === 0 && action !== "append" && action !== "focus") { return; }
        const step = actions[action];
        if (step === undefined) { throw new Error("Unknown desk action"); }
        step(ids);
        status.setText("Rows: " + rows.length);
    }
};
scenario.seed([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
for (const action of ["append", "edit", "reorder", "remove"]) {
    document.getElementById(action).addEventListener("click", () => {
        let index = sequence;
        if (action === "edit") { index = 0; }
        scenario.run(action, [index]);
    });
}
