import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://network/");
const window = platform.window({title: "Plectron - Network Resources", width: 720, height: 640});
window.on("error", error => console.log("error", error));
window.on("close", () => server.close());
window.load("app://network/network/index.html");
