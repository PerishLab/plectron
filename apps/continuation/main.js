import { document } from "@plectron/engine";
let narrow = true;
let anywhere = false;
const word = document.getElementById("word");
const paragraph = document.getElementById("paragraph");
document.getElementById("width").addEventListener("click", () => {
    narrow = !narrow;
    if (narrow) { word.setAttribute("style", "width: 64px;"); }
    else { word.setAttribute("style", "width: 180px;"); }
});
document.getElementById("mode").addEventListener("click", () => {
    anywhere = !anywhere;
    if (anywhere) { paragraph.setAttribute("style", "width: 110px; overflow-wrap: anywhere;"); }
    else { paragraph.setAttribute("style", "width: auto; overflow-wrap: normal;"); }
});
