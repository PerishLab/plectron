import { platform, console } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://clusters/");
const window = platform.window({title: "Plectron - Cluster Atlas", width: 720, height: 640});
window.on("error", error => console.log("error", error));
window.on("close", () => server.close());
window.load("app://clusters/clusters/src/index.html");
