import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://todo/");

const window = platform.window({
    title: "Plectron Todo",
    width: 760,
    height: 680
});

window.on("ready", function () {
    console.log("todo ready");
});

window.on("message", function (message) {
    console.log("todo", message);
});

window.on("error", function (error) {
    window.title("Plectron Todo - content failed");
    console.log("todo error", error);
});

window.on("close", function () {
    server.close();
});

window.load("app://todo/todo/index.html");
