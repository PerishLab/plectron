import { document, bridge } from "@plectron/engine";
import Boundary from "@plectron/components/boundary";
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
        bridge.postMessage("boundary:" + failure.phase + ":" + failure.primary.message);
    }
});
if (app !== null) { app.focus(); }
