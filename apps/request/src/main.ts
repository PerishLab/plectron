import { document, bridge, fetch, requests } from "@plectron/engine";
const url = document.getElementById("url");
const status = document.getElementById("status");
const result = document.getElementById("result");
const ping = document.getElementById("ping");
url.setValue("./message.txt");
let clicks = 0;
let active: ReturnType<typeof requests.scope> | undefined = undefined;
ping.addEventListener("click", () => { clicks++; ping.setText("Clicks: " + clicks); });
document.getElementById("cancel").addEventListener("click", () => {
    if (active !== undefined) { active.cancel(); }
});
document.getElementById("load").addEventListener("click", async () => {
    if (active !== undefined) { active.cancel(); }
    const request = requests.scope();
    active = request;
    status.setText("Loading");
    try {
        const response = await request.fetch(url.getValue());
        const text = await response.text();
        if (active === request) {
            result.setText(text);
            status.setText("HTTP " + response.status);
            bridge.postMessage("received " + text.length + " code units");
        }
    } catch (error) {
        if (active === request) { status.setText(error.message); }
    } finally {
        if (active === request) { active = undefined; }
    }
});
