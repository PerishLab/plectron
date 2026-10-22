import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://atoms/");
const window = platform.window({title: "Plectron - Pictures in a Sentence", width: 720, height: 700});
window.on("error", error => console.log("error", error));
window.on("close", () => server.close());
window.load("app://atoms/atoms/src/index.html");
