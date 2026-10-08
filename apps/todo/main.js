import { document, bridge } from "@plectron/engine";
import Boundary from "@plectron/composition/boundary";
import { Scope } from "@plectron/reactive";
import App from "./app.js";

const scope = new Scope();
let app = null;
Boundary({
    document: document,
    host: document.getElementById("app"),
    scope: scope,
    content: (context) => {
        app = App({document: document, scope: scope, wrap: context.wrap});
        return app;
    },
    report: (failure) => {
        const error = failure.primary;
        let message = "";
        if (error instanceof Error) { message = error.message; }
        else { message = String(error); }
        bridge.postMessage("boundary:" + failure.phase + ":" + message);
    }
});
if (app !== null) { app.focus(); }
