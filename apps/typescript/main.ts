import { document, bridge, events } from "@plectron/engine";
import template from "./view.pt";
import type { State } from "./model.ts";
import { create, fail, later } from "./model.ts";

const state: State = create();
const view = template(document);
const scope = events.scope();
view.listen({
    increment: function () {
        state.count += 1;
        view.refs.count.setText("Count: " + state.count);
        bridge.postMessage("count:" + state.count);
    },
    failure: fail,
    asynchronous: function () {
        later().then();
    }
}, scope);
document.getElementById("page").appendChild(view.host);
bridge.onMessage(function (message: string) {
    if (message === "increment") {
        view.refs.increment.dispatchEvent("click", "");
    } else if (message === "failure") {
        view.refs.failure.dispatchEvent("click", "");
    } else if (message === "asynchronous") {
        view.refs.asynchronous.dispatchEvent("click", "");
    } else if (message === "dispose") {
        scope.dispose();
        document.getElementById("page").removeChild(view.host);
    }
});
