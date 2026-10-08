import { platform, console } from "@plectron/engine";

const server = platform.serve("apps");
server.scheme("app://typescript/");
const window = platform.window({ title: "Plectron TypeScript", width: 640, height: 400 });
window.on("message", function (message: string) {
    console.log("typescript", message);
});
window.on("error", function (error: string) {
    console.log("typescript error", error);
});
window.on("close", function () {
    server.close();
});
window.load("app://typescript/typescript/index.html");
