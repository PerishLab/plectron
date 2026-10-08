import { document, bridge } from "@plectron/engine";
let count = 0;
const label = document.getElementById("count");
const increment = document.getElementById("increment");
const status = document.getElementById("status");

const update = function () {
    label.setText("Count: " + count);
    if (count % 2 === 0) {
        increment.setAttribute("style", "background-color: #226655; height: 24px;");
    } else {
        increment.setAttribute("style", "background-color: #334f99; height: 40px;");
    }
    bridge.postMessage("count: " + count);
};

increment.addEventListener("click", function (event) {
    count = count + 1;
    update();
});

document.getElementById("reset").addEventListener("click", function (event) {
    count = 0;
    update();
});

document.getElementById("page").addEventListener("click", function (event) {
    status.setText("Click bubbled to the page.");
});

bridge.onMessage(function (message) {
    if (message === "reset") {
        count = 0;
        update();
    }
});
