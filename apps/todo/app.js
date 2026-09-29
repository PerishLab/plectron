import { events, bridge } from "@plectron/engine";
import Input from "@plectron/components/input";
import Row from "./row.js";
import Template from "./app.pt";

export default function App(context) {
    const view = Template(context.document);
    const host = view.host;
    const refs = view.refs;
    const wrap = context.wrap;
    const editor = Input(context.document);
    const input = editor.host;
    input.setAttribute("id", "task-input");
    input.setAttribute("class", "editor grow p-3 bg-input");
    refs.composer.appendChild(input);
    refs.composer.appendChild(refs.add);
    const scope = context.scope;
    const state = scope.reactive({
        items: [
            {id: 1, title: "Shape the interaction boundary", done: false},
            {id: 2, title: "Try the application with a keyboard", done: false},
            {id: 3, title: "Record what feels unclear", done: true}
        ],
        filter: "all"
    });
    let instances = [];
    let sequence = 4;
    const subscription = events.scope();

    const remaining = scope.computed(() => {
        let remaining = 0;
        for (const item of state.items) {
            if (!item.done) { remaining = remaining + 1; }
        }
        return remaining;
    });
    scope.effect(() => {
        refs.summary.setText(remaining.value + " remaining / " + state.items.length + " total");
    });
    const status = (text) => refs.status.setText(text);
    const retire = (item) => {
        const retained = [];
        for (const entry of instances) {
            if (entry.item === item) {
                entry.instance.dispose();
                refs.tasks.removeChild(entry.instance.host);
            } else { retained[retained.length] = entry; }
        }
        instances = retained;
    };
    const remove = (item) => {
        retire(item);
        const retained = [];
        for (const candidate of state.items) {
            if (candidate !== item) { retained[retained.length] = candidate; }
        }
        state.items = retained;
        status("Deleted “" + item.title + "”");
        bridge.postMessage("deleted:" + item.id);
    };
    const mount = (item) => {
        const instance = Row({
            document: context.document,
            wrap: wrap,
            props: {
                item: item,
                filter: () => state.filter,
                scope: scope,
                status: status,
                remove: remove
            }
        });
        instances[instances.length] = {item: item, instance: instance};
        refs.tasks.appendChild(instance.host);
    };
    scope.effect(() => {
        let allClass = "filter p-2 bg-muted";
        let activeClass = "filter p-2 bg-muted";
        let completedClass = "filter p-2 bg-muted";
        if (state.filter === "all") { allClass = "filter p-2 bg-selected"; }
        if (state.filter === "active") { activeClass = "filter p-2 bg-selected"; }
        if (state.filter === "completed") { completedClass = "filter p-2 bg-selected"; }
        refs.all.setAttribute("class", allClass);
        refs.active.setAttribute("class", activeClass);
        refs.completed.setAttribute("class", completedClass);
    });
    const add = () => {
        const title = input.getValue();
        if (title === "") {
            status("Type a task before adding it");
            input.focus();
            return;
        }
        const item = {id: sequence, title: title, done: false};
        sequence = sequence + 1;
        state.items = [...state.items, item];
        mount(state.items[state.items.length - 1]);
        input.setValue("");
        input.focus();
        status("Added “" + title + "”");
        bridge.postMessage("added:" + item.id);
    };
    const show = (mode) => {
        state.filter = mode;
        status("Showing " + mode + " tasks");
    };
    const addfocus = () => {
        refs.add.setAttribute("class", "primary p-3 bg-selected");
    };
    const addblur = () => {
        refs.add.setAttribute("class", "primary p-3 bg-primary text-light");
    };
    const showall = () => show("all");
    const showactive = () => show("active");
    const showcompleted = () => show("completed");
    const clear = () => {
        const retained = [];
        for (const item of state.items) {
            if (item.done) {
                retire(item);
            } else {
                retained[retained.length] = item;
            }
        }
        state.items = retained;
        status("Cleared completed tasks");
        bridge.postMessage("cleared:" + state.items.length);
    };
    for (const item of state.items) { mount(item); }
    input.addEventListener("focus", wrap(() => {
        input.setAttribute("class", "editor grow p-3 bg-selected");
    }), {scope: subscription});
    input.addEventListener("blur", wrap(() => {
        input.setAttribute("class", "editor grow p-3 bg-input");
    }), {scope: subscription});
    const addentry = wrap(add);
    input.addEventListener("commit", addentry, {scope: subscription});
    view.listen({
        addfocus: wrap(addfocus),
        addblur: wrap(addblur),
        add: addentry,
        showall: wrap(showall),
        showactive: wrap(showactive),
        showcompleted: wrap(showcompleted),
        clear: wrap(clear)
    }, subscription);
    let disposed = false;
    const dispose = () => {
        if (disposed) { return; }
        disposed = true;
        subscription.dispose();
        for (const entry of instances) { entry.instance.dispose(); }
    };
    return {host: host, focus: () => input.focus(), dispose: dispose};
};
