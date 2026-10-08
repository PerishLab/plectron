import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://scroll/");
const window = platform.window({ title: "Plectron - scroll the scene", width: 360, height: 240 });
window.on("ready", function () { console.log("scroll ready"); });
window.on("message", function (message) { window.title(message); });
window.on("error", function (error) { console.log(error); });
window.on("close", function () { server.close(); });
window.load("app://scroll/scroll/index.html");
