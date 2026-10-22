import { platform } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://lifecycle/");

const window = platform.window({
    title: "Plectron Component Lifecycle",
    width: 640,
    height: 320
});

window.on("close", function () {
    server.close();
});

window.load("app://lifecycle/lifecycle/src/index.html");
