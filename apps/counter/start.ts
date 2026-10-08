import { platform, console } from "@plectron/engine";
const window = platform.window({
    title: "Plectron — click to count",
    width: 640,
    height: 400
});

let count = 0;
window.fill(1581872);

window.on("click", function () {
    count = count + 1;
    window.title("Plectron — clicks: " + count);
    if (count % 2 === 0) {
        window.fill(1581872);
    } else {
        window.fill(2253947);
    }
    console.log("click", count);
});

window.on("close", function () {
    console.log("closed after", count, "clicks");
});
