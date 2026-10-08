import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://typography/");
const window = platform.window({title: "Plectron - Text Lab", width: 720, height: 640});
window.on("error", error => console.log("error", error));
window.on("close", () => server.close());
window.load("app://typography/typography/index.html");
