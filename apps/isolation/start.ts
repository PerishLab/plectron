import { platform } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://isolation/");
const window = platform.window({title: "Plectron - isolated layout", width: 720, height: 640});
window.on("close", () => server.close());
window.load("app://isolation/isolation/index.html");
