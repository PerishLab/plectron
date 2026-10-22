import { document } from "@plectron/engine";
let dusk = false;
let small = false;
let faded = false;
const landscape = document.getElementById("landscape");
const detail = document.getElementById("detail");
const status = document.getElementById("status");
const change = () => {
    dusk = !dusk;
    for (const image of [landscape, detail]) {
        if (dusk) {image.setAttribute("src", "./dusk.png");}
        else {image.setAttribute("src", "./dawn.png");}
    }
    if (dusk) {status.setText("Evening arrives. Both views follow the light.");}
    else {status.setText("Morning returns. The first picture is still here.");}
};
landscape.addEventListener("click", change);
document.getElementById("swap").addEventListener("click", change);
document.getElementById("size").addEventListener("click", () => {
    small = !small;
    if (small) {landscape.setAttribute("style", "width: 210px;");}
    else {landscape.setAttribute("style", "width: 294px;");}
});
document.getElementById("fade").addEventListener("click", () => {
    faded = !faded;
    if (faded) {detail.setAttribute("style", "opacity: 0.45;");}
    else {detail.setAttribute("style", "opacity: 1;");}
});
