import { document, bridge } from "@plectron/engine";
const board = document.getElementById("board");
const status = document.getElementById("status");
const count = document.getElementById("count");
let panels = [];
let sequence = 0;
let selected = "";

const element = function (tag, id, kind, text) {
    const node = document.createElement(tag);
    node.setAttribute("id", id);
    node.setAttribute("class", kind);
    if (text !== "") {
        node.setText(text);
    }
    return node;
};

const add = function () {
    if (panels.length === 3) {
        return;
    }
    const id = sequence;
    sequence = sequence + 1;
    const panel = element("div", "panel-" + id, "panel", "");
    panel.appendChild(element("h2", "title-" + id, "title", "Panel " + id));
    const rows = element("div", "rows-" + id, "rows", "");
    for (const index of [0, 1, 2, 3, 4, 5]) {
        const name = id + ":" + index;
        const row = element("button", "row-" + id + "-" + index, "row", "Item " + index);
        row.addEventListener("click", function (event) {
            selected = name;
            row.setAttribute("style", "background-color: #0e7490;");
            status.setText("Selected " + name);
            bridge.postMessage("selected:" + name);
        });
        rows.appendChild(row);
    }
    panel.appendChild(rows);
    panel.appendChild(element("p", "footer-" + id, "footer", "6 items"));
    board.appendChild(panel);
    panels[panels.length] = panel;
    count.setText(panels.length + " panels");
};

const remove = function () {
    if (panels.length === 1) {
        return;
    }
    board.removeChild(panels[panels.length - 1]);
    panels.length = panels.length - 1;
    count.setText(panels.length + " panels");
};

document.getElementById("add").addEventListener("click", function (event) { add(); });
document.getElementById("remove").addEventListener("click", function (event) { remove(); });
add();
add();
