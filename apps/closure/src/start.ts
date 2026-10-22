import { platform, console } from "@plectron/engine";
const create = function (label: string, color: number) {
    const window = platform.window({ title: label, width: 400, height: 260 });
    let count = 0;
    window.fill(color);
    window.on("click", function () {
        count = count + 1;
        window.title(label + ": " + count);
        if (count === 3) {
            window.close();
        }
    });
    window.on("close", function () {
        console.log(label, "closed at", count);
    });
};

create("First — 3 clicks to close", 2253947);
create("Second — 3 clicks to close", 6572110);
