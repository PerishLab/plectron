import { document as scenarioDocument } from "@plectron/engine";
const actions = {
    append: (ids) => {
        for (const id of ids) {
            const item = {id: id, title: "Task " + id, done: false};
            items[items.length] = item;
            mount(item);
            sequence = id + 1;
        }
    },
    edit: (ids) => {
        for (const index of ids) {
            items[index].title = "Changed task";
            if (index !== 0) { items[index].title = items[index].title + " " + index; }
            update(items[index]);
        }
    },
    reorder: () => {
        const first = items[0];
        const rest = [];
        for (const item of items) {
            if (item !== first) { rest[rest.length] = item; }
        }
        rest[rest.length] = first;
        items = rest;
        list.appendChild(first.node);
    },
    remove: () => {
        const first = items[0];
        first.scope.dispose();
        list.removeChild(first.node);
        const rest = [];
        for (const item of items) {
            if (item !== first) { rest[rest.length] = item; }
        }
        items = rest;
    }
};
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
        const step = actions[action];
        if (step === undefined) { throw new Error("Unknown list action"); }
        step(ids);
        status.setText("Tasks: " + items.length);
    }
};
scenarioDocument.getElementById("reorder").addEventListener("click", () => scenario.run("reorder", []));
