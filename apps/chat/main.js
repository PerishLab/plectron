import { document } from "@plectron/engine";
import App from "./app.js";

const app = App(document);
const scenario = app.scenario;
document.getElementById("app").appendChild(app.host);
app.focus();
