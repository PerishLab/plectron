import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://tasks/");

const window = platform.window({
    title: "Plectron - loading",
    width: 720,
    height: 640
});

window.on("ready", function () {
    window.title("Plectron - app://tasks/list/src/index.html");
    console.log("ready", "app://tasks/list/src/index.html");
});

window.on("message", function (message) {
    window.title("Plectron - " + message);
    console.log("page", message);
});

window.on("error", function (error) {
    window.title("Plectron - content failed");
    console.log("content error", error);
});

window.on("close", function () {
    server.close();
    console.log("page and static server closed");
});

window.load("app://tasks/list/src/index.html");
