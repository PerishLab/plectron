import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://reshaping/");
const window = platform.window({title: "Plectron - Ligatures Across Lines", width: 720, height: 640});
window.on("error", error => console.log("error", error));
window.on("close", () => server.close());
window.load("app://reshaping/reshaping/index.html");
