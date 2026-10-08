import { document } from "@plectron/engine";
let narrow = false;
let green = false;
let selected = "";
const status = document.getElementById("status");
const sentence = document.getElementById("sentence");
const tone = document.getElementById("tone");
for (const name of ["ligature", "arabic"]) {
    document.getElementById(name).addEventListener("click", () => {
        selected = name;
        status.setText("You selected the blue letters in the " + name + " sample.");
    });
}
tone.addEventListener("click", () => {
    green = !green;
    if (green) {tone.setAttribute("class", "green");}
    else {tone.setAttribute("class", "blue");}
    status.setText("A new tone. The same sentence.");
});
document.getElementById("measure").addEventListener("click", () => {
    narrow = !narrow;
    if (narrow) {sentence.setAttribute("style", "width: 340px;");}
    else {sentence.setAttribute("style", "width: 620px;");}
});
