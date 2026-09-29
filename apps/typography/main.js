import { document } from "@plectron/engine";
const preview = document.getElementById("preview");
const size = document.getElementById("size");
const leading = document.getElementById("leading");
const align = document.getElementById("align");
let large = false;
let tight = false;
let centered = false;
const update = () => {
    let pixels = 20;
    if (large) { pixels = 28.5; }
    let height = 1.5;
    if (tight) { height = 0.85; }
    let alignment = "start";
    if (centered) { alignment = "center"; }
    preview.setAttribute("style", "font-size: " + pixels + "px; line-height: " + height + "; text-align: " + alignment + ";");
    size.setText("Size: " + pixels);
    leading.setText("Leading: " + height);
    align.setText("Align: " + alignment);
};
size.addEventListener("click", () => { large = !large; update(); });
leading.addEventListener("click", () => { tight = !tight; update(); });
align.addEventListener("click", () => { centered = !centered; update(); });
document.getElementById("editor").setValue("Café — edit this line");
