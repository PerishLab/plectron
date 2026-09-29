import { document } from "@plectron/engine";
let narrow = true;
let alternate = false;
const latin = document.getElementById("latin");
const arabic = document.getElementById("arabic");
const whole = document.getElementById("whole");
document.getElementById("width").addEventListener("click", () => {
    narrow = !narrow;
    for (const sample of [latin, arabic]) {
        if (narrow) { sample.setAttribute("style", "width: 16px;"); }
        else { sample.setAttribute("style", "width: 180px;"); }
    }
});
document.getElementById("word").addEventListener("click", () => {
    alternate = !alternate;
    for (const sample of [arabic, whole]) {
        if (alternate) { sample.setText("بلا"); }
        else { sample.setText("لَا"); }
    }
});
