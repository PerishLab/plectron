import { document as scenarioDocument } from "@plectron/engine";
const scenario = {
    seed: (ids) => {
        for (const item of items) {
            item.scope.dispose();
            list.removeChild(item.node);
        }
        items = [];
        scenario.run("append", ids);
    },
    run: (action, ids) => {
        if (items.length === 0 && action !== "append" && action !== "focus") { return; }
        if (action === "append") {
            for (const id of ids) {
                const item = {id: id, title: "Task " + id, done: false};
                items[items.length] = item;
                mount(item);
                sequence = id + 1;
            }
        } else if (action === "edit") {
            for (const index of ids) {
                items[index].title = "Changed task";
                if (index !== 0) { items[index].title = items[index].title + " " + index; }
                update(items[index]);
            }
        } else if (action === "reorder") {
            const first = items[0];
            const rest = [];
            for (const item of items) {
                if (item !== first) { rest[rest.length] = item; }
            }
            rest[rest.length] = first;
            items = rest;
            list.appendChild(first.node);
        } else if (action === "remove") {
            const first = items[0];
            first.scope.dispose();
            list.removeChild(first.node);
            const rest = [];
            for (const item of items) {
                if (item !== first) { rest[rest.length] = item; }
            }
            items = rest;
        } else { throw new Error("Unknown list action"); }
        status.setText("Tasks: " + items.length);
    }
};
scenarioDocument.getElementById("reorder").addEventListener("click", () => scenario.run("reorder", []));
