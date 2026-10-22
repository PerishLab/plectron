import { document } from "@plectron/engine";
import App from "./app.ts";

const app = App(document);
const scenario = app.scenario;
document.getElementById("app").appendChild(app.host);
app.focus();
