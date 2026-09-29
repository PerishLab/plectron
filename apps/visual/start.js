import { platform } from "@plectron/engine";

const server = platform.serve("apps");
server.scheme("app://visual/");

const window = platform.window({
    title: "Plectron Visual",
    width: 720,
    height: 640
});

window.on("close", function () {
    server.close();
});

window.load("app://visual/visual/index.html");
