import { document } from "@plectron/engine";
const paragraphs = [document.getElementById("chinese"), document.getElementById("japanese"), document.getElementById("korean"), document.getElementById("mixed")];
let narrow = false;
document.getElementById("width").addEventListener("click", () => {
    narrow = !narrow;
    for (const paragraph of paragraphs) {
        if (narrow) { paragraph.setAttribute("style", "width: 220px;"); }
        else { paragraph.setAttribute("style", "width: auto;"); }
    }
});
let keep = true;
document.getElementById("words").addEventListener("click", () => {
    keep = !keep;
    if (keep) { paragraphs[2].setAttribute("class", "sample korean"); }
    else { paragraphs[2].setAttribute("class", "sample korean loose"); }
});
