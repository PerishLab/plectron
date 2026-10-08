import { document, bridge, scroll } from "@plectron/engine";
let clicks = 0;
let last = "none";
const scrolling = scroll.attach(document.getElementById("inner"));
const status = document.getElementById("status");
for (const name of ["before", "a", "b", "c", "after"]) {
    const button = document.getElementById(name);
    button.addEventListener("click", function (event) {
        clicks = clicks + 1;
        last = name;
        status.setText(name + " / " + clicks);
        bridge.postMessage(name + " / " + clicks);
    });
}
