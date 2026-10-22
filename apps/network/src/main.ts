import { document } from "@plectron/engine";
const picture = document.getElementById("picture");
let dusk = false;
document.getElementById("swap").addEventListener("click", () => {
    dusk = !dusk;
    if (dusk) {
        picture.setAttribute("src", "../../pictures/src/dusk.png");
        document.getElementById("status").setText("Dusk loaded. Font resources retained.");
    } else {
        picture.setAttribute("src", "../../pictures/src/dawn.png");
        document.getElementById("status").setText("Dawn restored. Image identity reused.");
    }
});
