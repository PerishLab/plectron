import { platform } from "@plectron/engine";

const server = platform.serve("apps");
server.scheme("app://chat/");

const window = platform.window({
    title: "Plectron Chat",
    width: 1280,
    height: 800
});

window.on("close", function () {
    server.close();
});

window.load("app://chat/chat/src/index.html");
