import { events, requests } from "@plectron/engine";
import Input from "@plectron/composition/input";
import Scroll from "@plectron/composition/scroll";
import Channel from "./channel.ts";
import Message from "./message.pt";
import Template from "./app.pt";
import type { Document, Message as Record, Name, Node, Position } from "./model.ts";

const messages = [
    0, 1, 2, 3, 4, 5, 6, 7,
    8, 9, 10, 11, 12, 13, 14, 15,
    16, 17, 18, 19, 20, 21, 22, 23,
    24, 25, 26, 27, 28, 29, 30, 31
];

export default function App(document: Document) {
    const view = Template(document);
    const editor = Input(document);
    const channels = {general: Channel("general"), engine: Channel("engine")};
    channels.general.seed(messages);
    channels.engine.seed(messages);
    let channel = channels.general;
    const latest = view.refs.latest;
    const history = view.refs.history;
    const show = () => {
        latest.setText(channel.pending() + " new messages ↓");
        latest.setAttribute("class", "latest");
    };
    const hide = () => {
        channel.read();
        latest.setAttribute("class", "latest latesthidden");
    };
    const timeline = Scroll<Node, string, Node>(view.refs.timeline, (state) => {
        if (state.edge === "end") { hide(); }
    }, (node) => node.getAttribute("id"));
    const resume = () => {
        hide();
        timeline.followEnd();
    };
    const input = editor.host;
    const subscription = events.scope();
    let rows: ReturnType<typeof create>[] = [];
    let pending: ReturnType<typeof requests.scope> | undefined = undefined;
    let generation = 0;
    const meta = () => view.refs.meta.setText(channel.count() + " messages · 8 members");

    input.setAttribute("id", "message-input");
    input.setAttribute("class", "message-input");
    view.refs.editor.appendChild(input);

    const create = (record: Record) => {
        const message = Message(document);
        message.host.setAttribute("id", "chat-message-" + (record.id + 1));
        message.refs.avatar.setText(record.avatar);
        message.refs.author.setText(record.author);
        message.refs.time.setText(record.time);
        message.refs.body.setAttribute("id", "chat-record-" + record.id);
        message.refs.body.setText(record.body);
        return {record: record, message: message};
    };
    const mount = (record: Record) => {
        const row = create(record);
        timeline.host.appendChild(row.message.host);
        rows[rows.length] = row;
    };
    for (const record of channel.items()) { mount(record); }
    timeline.followEnd();

    const cancel = () => {
        generation = generation + 1;
        if (pending !== undefined) { pending.cancel(); }
        pending = undefined;
        history.setText("Load history");
        history.setAttribute("class", "history");
    };
    const load = async () => {
        if (pending !== undefined) { pending.cancel(); }
        generation = generation + 1;
        const admitted = generation;
        const request = requests.scope();
        const owner = channel;
        pending = request;
        history.setText("Loading…");
        history.setAttribute("class", "history historyloading");
        try {
            const response = await request.fetch("./history.json");
            if (!response.ok) { throw new Error("history request failed"); }
            const records = await response.json();
            if (admitted !== generation || pending !== request || channel !== owner) { return; }
            const added = channel.prepend(records);
            const staged = [];
            for (const record of added) { staged[staged.length] = create(record); }
            const previous = rows;
            for (const row of staged) { timeline.host.appendChild(row.message.host); }
            for (const row of previous) { timeline.host.appendChild(row.message.host); }
            rows = staged;
            for (const row of previous) { rows[rows.length] = row; }
            meta();
            history.setText("Load history");
            history.setAttribute("class", "history");
        } catch (error) {
            if (admitted === generation && pending === request) {
                history.setText("Retry history");
                history.setAttribute("class", "history historyfailed");
            }
        } finally {
            if (admitted === generation && pending === request) {
                pending = undefined;
            }
        }
    };

    const select = (next: Name) => {
        const target = channels[next];
        if (channel === target) { input.focus(); return; }
        cancel();
        channel.write(input.getValue());
        channel.remember(timeline.bookmark());
        for (const row of rows) { timeline.host.removeChild(row.message.host); }
        rows = [];
        channel = target;
        let general = "channel";
        let engine = "channel";
        let subtitle = "Rendering, geometry, and runtime implementation notes.";
        if (next === "general") {
            general = "channel channelactive";
            subtitle = "Daily notes from the Plectron runtime workshop.";
        } else { engine = "channel channelactive"; }
        view.refs.general.setAttribute("class", general);
        view.refs.engine.setAttribute("class", engine);
        view.refs.title.setText("# " + next);
        view.refs.subtitle.setText(subtitle);
        for (const record of channel.items()) { mount(record); }
        input.setValue(channel.draft());
        meta();
        const restored = timeline.restore(channel.position());
        if (!restored) { timeline.followEnd(); }
        hide();
        input.focus();
    };

    const submit = () => {
        const text = input.getValue();
        if (text === "") { return; }
        const state = timeline.state();
        const detached = state !== null && state.edge !== "end";
        mount(channel.send(text));
        if (detached) {
            channel.notify(1);
            show();
        }
        input.setValue("");
        channel.write("");
        input.focus();
        meta();
    };
    const receive = (ids: number[]) => {
        const state = timeline.state();
        const detached = state !== null && state.edge !== "end";
        for (const record of channel.receive(ids)) { mount(record); }
        if (detached) {
            channel.notify(ids.length);
            show();
        } else { timeline.followEnd(); }
        meta();
    };
    input.addEventListener("commit", submit, {scope: subscription});
    history.addEventListener("click", load, {scope: subscription});
    latest.addEventListener("click", resume, {scope: subscription});
    view.refs.general.addEventListener("click", () => select("general"), {scope: subscription});
    view.refs.engine.addEventListener("click", () => select("engine"), {scope: subscription});

    const positions = (bookmark: Position) => {
        if (bookmark === null) { return "none"; }
        if (bookmark.edge === "end") { return "end"; }
        let value = bookmark.key + ":" + bookmark.inset;
        for (const fallback of bookmark.fallbacks) {
            value = value + " | " + fallback.key + ":" + fallback.inset;
        }
        return value;
    };
    const actions = {
        append: (ids: number[]) => {
            for (const record of channel.append(ids)) { mount(record); }
        },
        edit: (ids: number[]) => {
            for (const index of ids) {
                const body = "Changed message " + index + " now wraps across several lines to exercise variable geometry correction in the active conversation.";
                channel.edit(index, body);
                rows[index].message.refs.body.setText(body);
            }
        },
        reorder: () => {
            if (rows.length === 0) { return false; }
            const first = rows[0];
            const rest = [];
            for (const row of rows) { if (row !== first) { rest[rest.length] = row; } }
            rest[rest.length] = first;
            rows = rest;
            channel.move();
            timeline.host.appendChild(first.message.host);
        },
        remove: () => {
            if (rows.length === 0) { return false; }
            const first = rows[0];
            timeline.host.removeChild(first.message.host);
            const rest = [];
            for (const row of rows) { if (row !== first) { rest[rest.length] = row; } }
            rows = rest;
            channel.remove();
        },
        receive: (ids: number[]) => {
            receive(ids);
        },
        resume: () => {
            resume();
        },
        switch: () => {
            let next: Name = "general";
            if (channel.name === "general") { next = "engine"; }
            select(next);
        }
    };
    const scenario = {
        position: () => {
            const bookmark = timeline.bookmark();
            if (bookmark === null) { return "none"; }
            if (bookmark.edge === "end") { return "end"; }
            return bookmark.key + ":" + bookmark.inset;
        },
        positions: () => positions(timeline.bookmark()),
        saved: (name: Name) => positions(channels[name].position()),
        drop: (name: Name, key: string) => channels[name].drop(key),
        seed: (ids: number[]) => {
            for (const row of rows) { timeline.host.removeChild(row.message.host); }
            rows = [];
            channels.general.seed(ids);
            channels.engine.seed(ids);
            for (const record of channel.items()) { mount(record); }
            meta();
            timeline.followEnd();
        },
        run: (action: string, ids: number[]) => {
            const step = actions[action];
            if (step === undefined) { throw new Error("Unknown chat action"); }
            if (step(ids) === false) { return; }
            meta();
        }
    };

    let disposed = false;
    const dispose = () => {
        if (disposed) { return; }
        disposed = true;
        cancel();
        subscription.dispose();
        timeline.dispose();
    };
    return {host: view.host, focus: () => input.focus(), scenario: scenario, dispose: dispose};
};
