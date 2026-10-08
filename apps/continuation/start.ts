import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://continuation/");
const window = platform.window({title: "Plectron - Words Across Lines", width: 720, height: 640});
window.on("error", error => console.log("error", error));
window.on("close", () => server.close());
window.load("app://continuation/continuation/index.html");
