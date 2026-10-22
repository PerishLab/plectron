import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://pictures/");
const window = platform.window({title: "Plectron - Views Worth Keeping", width: 720, height: 640});
window.on("error", error => console.log("error", error));
window.on("close", () => server.close());
window.load("app://pictures/pictures/src/index.html");
