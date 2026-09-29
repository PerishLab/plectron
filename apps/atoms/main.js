import { document } from "@plectron/engine";
let narrow = false;
let large = false;
let dusk = false;
const sentence = document.getElementById("sentence");
const icon = document.getElementById("icon");
const view = document.getElementById("view");
const status = document.getElementById("status");
const grow = () => {
    large = !large;
    if (large) {icon.setAttribute("style", "width: 60px;");}
    else {icon.setAttribute("style", "width: 26px;");}
    status.setText("The sign grows. Its neighbors find their baseline.");
};
icon.addEventListener("click", grow);
document.getElementById("scale").addEventListener("click", grow);
document.getElementById("measure").addEventListener("click", () => {
    narrow = !narrow;
    if (narrow) {sentence.setAttribute("style", "width: 390px;");}
    else {sentence.setAttribute("style", "width: 620px;");}
    status.setText("The measure changes. The sentence stays together.");
});
document.getElementById("swap").addEventListener("click", () => {
    dusk = !dusk;
    if (dusk) {view.setAttribute("src", "dusk.png");}
    else {view.setAttribute("src", "dawn.png");}
    status.setText("A different light, in the same place.");
});
