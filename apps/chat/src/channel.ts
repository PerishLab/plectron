import type { Message, Name, Position } from "./model.ts";

const people = [
    {name: "Lin", avatar: "林"},
    {name: "Avery", avatar: "A"},
    {name: "Mika", avatar: "M"},
    {name: "Zhou", avatar: "周"},
    {name: "Sora", avatar: "S"},
    {name: "Kai", avatar: "K"}
];

const lines = [
    "The static shell is up. I am checking the first layout trace now.",
    "中文字体看起来很稳定，长句换行也没有跳动。",
    "I pushed the geometry notes into the channel topic for today.",
    "The left rail stays compact while the timeline receives the remaining width.",
    "Can we keep this message intentionally longer? It gives the line breaker enough text to form a second visual line inside the conversation column.",
    "可以，这一条也保留中英文 mixed content，方便观察 glyph shaping 和 baseline。",
    "The member list is quiet, but it still participates in the same top-level flex layout.",
    "✨ First frame, stable regions, and one scrollable timeline. That is enough for this slice.",
    "I like the density. It feels closer to a working room than a component showcase.",
    "下一步再引入频道切换，现在先让静态边界完全清楚。",
    "The composer now owns a real editor and appends one message per commit.",
    "Recorded. The current target is a trustworthy static product surface."
];

const without = (records: Message[], first: Message) => {
    const rest: Message[] = [];
    for (const record of records) {
        if (record !== first) { rest[rest.length] = record; }
    }
    return rest;
};

const matching = (records: Message[], key: string) => {
    const kept: Message[] = [];
    let removed = false;
    for (const record of records) {
        if ("chat-message-" + (record.id + 1) === key) { removed = true; }
        else { kept[kept.length] = record; }
    }
    return {records: kept, removed: removed};
};

export default function Channel(name: Name) {
    let records: Message[] = [];
    let sequence = 0;
    let draft = "";
    let unread = 0;
    let position: Position = null;
    const sample = (index: number) => {
        let shift = 0;
        if (name === "engine") { shift = 3; }
        const person = people[(index + shift) % people.length];
        let body = lines[(index + shift) % lines.length];
        if (name === "engine") { body = "Engine · " + body; }
        return {id: index, author: person.name, avatar: person.avatar, body: body, time: "09:" + (10 + index)};
    };
    const include = (record: Message) => {
        records[records.length] = record;
        if (sequence <= record.id) { sequence = record.id + 1; }
        return record;
    };
    return {
        name: name,
        items: () => records,
        count: () => records.length,
        seed: (ids: number[]) => {
            records = [];
            sequence = 0;
            position = null;
            for (const id of ids) { include(sample(id)); }
        },
        append: (ids: number[]) => {
            const added = [];
            for (const id of ids) { added[added.length] = include(sample(id)); }
            return added;
        },
        send: (body: string) => include({id: sequence, author: "You", avatar: "Y", body: body, time: "now"}),
        receive: (ids: number[]) => {
            const added = [];
            for (const ignored of ids) { added[added.length] = include(sample(sequence)); }
            return added;
        },
        prepend: (values: Omit<Message, "id">[]) => {
            const added = [];
            for (const value of values) {
                const record = {id: sequence, author: value.author, avatar: value.avatar, body: value.body, time: value.time};
                sequence = sequence + 1;
                added[added.length] = record;
            }
            const previous = records;
            const merged = [];
            for (const record of added) { merged[merged.length] = record; }
            for (const record of previous) { merged[merged.length] = record; }
            records = merged;
            return added;
        },
        edit: (index: number, body: string) => { records[index].body = body; },
        move: () => {
            if (records.length === 0) { return; }
            const first = records[0];
            const rest = without(records, first);
            rest[rest.length] = first;
            records = rest;
        },
        remove: () => {
            if (records.length === 0) { return; }
            const first = records[0];
            records = without(records, first);
        },
        drop: (key: string) => {
            const result = matching(records, key);
            records = result.records;
            return result.removed;
        },
        draft: () => draft,
        write: (value: string) => { draft = value; },
        position: () => position,
        remember: (value: Position) => { position = value; },
        pending: () => unread,
        notify: (count: number) => { unread = unread + count; },
        read: () => { unread = 0; }
    };
};
