import { document } from "@plectron/engine";
import Scroll from "@plectron/composition/scroll";
import Visual from "@plectron/composition/visual";
import Template from "./app.pt";
import Item from "./item.pt";

const total = 100000;
const view = Template(document);
const identity = (index) => "visual-record-" + index;
const create = (index) => {
    const item = Item(document);
    item.host.setAttribute("id", identity(index));
    if (index % 5 === 0) { item.host.setAttribute("class", "item tall"); }
    item.refs.key.setText("#" + index);
    item.refs.body.setText("Record " + index + " is materialized from an exact layout observation.");
    return item;
};
const timeline = Scroll(view.refs.viewport, undefined, (node) => node.getAttribute("id"));
const visual = Visual(view.refs.viewport, {
    count: () => total,
    key: identity,
    mount: create,
    batch: 16,
    margin: 256,
    anchor: "end",
    scroll: timeline
});
const status = () => {
    const range = visual.range();
    view.refs.status.setText(range.start + "–" + range.end + " live · " + range.total + " logical");
};
status();
const jump = (index) => {
    visual.reset(index);
    status();
};
const scenario = {
    count: () => total,
    range: () => {
        const range = visual.range();
        return range.start + ":" + range.end + ":" + range.total;
    },
    start: () => {
        visual.reset("start");
        status();
    },
    end: () => {
        visual.reset("end");
        status();
    },
    jump: jump,
    grow: (index) => {
        const item = visual.item(index);
        if (item === null) { throw new Error("visual item is not live"); }
        item.host.setAttribute("class", "item tall");
        status();
    }
};

document.getElementById("app").appendChild(view.host);
