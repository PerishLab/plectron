import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://inline/");
const window = platform.window({title: "Plectron - A Paragraph, Many Voices", width: 720, height: 640});
window.on("error", error => console.log("error", error));
window.on("close", () => server.close());
window.load("app://inline/inline/index.html");
