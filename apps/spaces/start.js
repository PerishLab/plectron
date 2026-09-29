import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://spaces/");
const window = platform.window({ title: "Plectron - spaces the scene", width: 360, height: 240 });
window.on("ready", function () { console.log("spaces ready"); });
window.on("message", function (message) { window.title(message); });
window.on("error", function (error) { console.log(error); });
window.on("close", function () { server.close(); });
window.load("app://spaces/spaces/index.html");
