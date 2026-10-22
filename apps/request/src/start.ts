import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://request/");
const window = platform.window({title: "Plectron - async requests", width: 640, height: 420});
window.on("message", message => window.title("Plectron - " + message));
window.on("error", error => console.log("error", error));
window.on("close", () => server.close());
window.load("app://request/request/src/index.html");
