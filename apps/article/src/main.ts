import { document } from "@plectron/engine";
const root = document.getElementById("article");
const status = document.getElementById("status");
const texts = ["Modern boundaries, office ffi, café é. A paragraph can change independently.", "中文排版与现代浏览器。文字簇、字体与段落，各有清晰边界。", "العربية لغة جميلة — office 123. النص يجمع لغات مختلفة."];
let paragraphs = [];
let sequence = 0;
let narrow = false;
let bold = false;
const actions = {
    append: (ids) => {
        for (const id of ids) {
            const node = document.createElement("p");
            node.setAttribute("id", "item-" + id);
            node.setAttribute("class", "paragraph");
            node.setText(texts[id % 3]);
            root.appendChild(node);
            paragraphs[paragraphs.length] = {id: id, node: node};
            sequence = id + 1;
        }
    },
    edit: (ids) => {
        for (const index of ids) {
            let text = "Changed paragraph: office ffi / 中文 / العربية.";
            if (index !== 0) { text = text + " " + index; }
            paragraphs[index].node.setText(text);
        }
    },
    reorder: () => {
        const first = paragraphs[0];
        const rest = [];
        for (const item of paragraphs) {
            if (item !== first) { rest[rest.length] = item; }
        }
        rest[rest.length] = first;
        paragraphs = rest;
        root.appendChild(first.node);
    },
    remove: () => {
        const first = paragraphs[0];
        root.removeChild(first.node);
        const rest = [];
        for (const item of paragraphs) {
            if (item !== first) { rest[rest.length] = item; }
        }
        paragraphs = rest;
    },
    width: () => {
        narrow = !narrow;
    },
    font: () => {
        bold = !bold;
    }
};
const scenario = {
    seed: (ids) => {
        for (const item of paragraphs) { root.removeChild(item.node); }
        paragraphs = [];
        scenario.run("append", ids);
    },
    run: (action, ids) => {
        const step = actions[action];
        if (step === undefined) { throw new Error("Unknown article action"); }
        step(ids);
        let style = "width: auto;";
        if (narrow) { style = "width: 360px;"; }
        if (bold) { style = style + "font-weight: bold;"; }
        root.setAttribute("style", style);
        status.setText("Paragraphs: " + paragraphs.length);
    }
};
scenario.seed([0, 1, 2, 3, 4, 5]);
for (const action of ["append", "edit", "width", "font"]) {
    document.getElementById(action).addEventListener("click", () => {
        let index = sequence;
        if (action === "edit") { index = 0; }
        scenario.run(action, [index]);
    });
}
