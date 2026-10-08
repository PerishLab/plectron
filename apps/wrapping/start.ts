import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://wrapping/");
const window = platform.window({title: "Plectron - Multilingual Paragraphs", width: 720, height: 640});
window.on("error", error => console.log("error", error));
window.on("close", () => server.close());
window.load("app://wrapping/wrapping/index.html");
