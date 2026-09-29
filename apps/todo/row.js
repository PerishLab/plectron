import { events, bridge } from "@plectron/engine";
import Template from "./row.pt";

export const rowClass = (item, filter, hovering) => {
    let name = "task-row flex items-center gap-2 p-2 bg-input";
    if (hovering) { name = "task-row flex items-center gap-2 p-2 bg-hover"; }
    if (item.done) { name = name + " task-done"; }
    if (filter === "active" && item.done) { name = name + " hidden"; }
    if (filter === "completed" && !item.done) { name = name + " hidden"; }
    return name;
};

export default function Row(context) {
    const view = Template(context.document);
    const host = view.host;
    const refs = view.refs;
    const props = context.props;
    const item = props.item;
    const subscription = events.scope();
    const hover = props.scope.reactive({active: false});
    host.setAttribute("id", "todo-" + item.id);
    refs.toggle.setAttribute("id", "toggle-" + item.id);
    refs.remove.setAttribute("id", "remove-" + item.id);
    refs.label.setText(item.title);
    const stopclass = props.scope.effect(() => {
        host.setAttribute("class", rowClass(item, props.filter(), hover.active));
    });
    const stoplabel = props.scope.effect(() => {
        if (item.done) { refs.toggle.setText("Completed"); }
        else { refs.toggle.setText("Complete"); }
    });
    const toggle = () => {
        item.done = !item.done;
        if (item.done) { props.status("Completed “" + item.title + "”"); }
        else { props.status("Reopened “" + item.title + "”"); }
        bridge.postMessage("changed:" + item.id);
    };
    const remove = () => props.remove(item);
    const enter = () => { hover.active = true; };
    const leave = () => { hover.active = false; };
    view.listen({
        toggle: context.wrap(toggle),
        remove: context.wrap(remove),
        enter: context.wrap(enter),
        leave: context.wrap(leave)
    }, subscription);
    let disposed = false;
    const dispose = () => {
        if (disposed) { return; }
        disposed = true;
        subscription.dispose();
        stopclass();
        stoplabel();
    };
    return {host: host, dispose: dispose};
};
