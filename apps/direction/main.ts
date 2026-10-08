import { document } from "@plectron/engine";
const mixed = document.getElementById("mixed");
const toggle = document.getElementById("toggle");
let reversed = true;
toggle.addEventListener("click", () => {
    reversed = !reversed;
    if (reversed) { mixed.setAttribute("dir", "rtl"); }
    else { mixed.setAttribute("dir", "ltr"); }
});
document.getElementById("editor").setValue("Edit سلام 123 — مرحباً");
