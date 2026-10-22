import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://demo/");

const window = platform.window({
    title: "Plectron - loading",
    width: 640,
    height: 480
});

window.on("ready", function () {
    window.title("Plectron - app://demo/site/src/index.html");
    console.log("ready", "app://demo/site/src/index.html");
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

window.load("app://demo/site/src/index.html");
