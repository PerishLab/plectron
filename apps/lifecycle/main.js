import { document } from "@plectron/engine";
import Message from "./message.pt";
import App from "./app.pt";

const app = App(document);
const message = Message(document);
app.host.appendChild(message.host);
document.getElementById("app").appendChild(app.host);
