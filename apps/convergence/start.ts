import { platform } from "@plectron/engine";
const server = platform.serve("apps");
server.scheme("app://convergence/");
const window = platform.window({title: "Plectron - layout convergence", width: 720, height: 640});
window.on("close", () => server.close());
window.load("app://convergence/convergence/index.html");
