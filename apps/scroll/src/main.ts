import { document, bridge } from "@plectron/engine";
let clicks = 0;
let last = "none";
const rows = document.getElementById("rows");
for (const name of ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth"]) {
    const button = document.createElement("button");
    button.setAttribute("id", name);
    button.setText("fi - " + name);
    button.addEventListener("click", function (event) {
        clicks = clicks + 1;
        last = name;
        bridge.postMessage(name + " / " + clicks);
    });
    rows.appendChild(button);
}
