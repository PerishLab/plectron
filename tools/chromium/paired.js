async page => {
    const config = __CONFIG__;
    const browser = page.context().browser();
    const context = await browser.newContext({ viewport: { width: 720, height: 640 }, deviceScaleFactor: 1 });
    const target = await context.newPage();
    const errors = [];
    const receipts = [];
    target.on("response", response => {
        if (!response.url().startsWith(config.url)) return;
        const path = response.url().slice(config.url.length).split("?")[0];
        if (Object.hasOwn(config.scripts, path)) receipts.push((async () => ({
            path, status: response.status(), mime: response.headers()["content-type"],
            body: (await response.body()).toString("base64")
        }))().catch(error => { errors.push(String(error)); return null; }));
    });
    for (const [path, body] of Object.entries(config.scripts)) {
        await target.route(config.url + path, route => route.fulfill({ status: 200, contentType: "text/javascript; charset=utf-8", body }));
    }
    target.on("pageerror", error => errors.push(String(error)));
    const reset = "*{margin:0;padding:0;border:0;box-sizing:content-box;caret-color:transparent}h1,button{font:inherit;text-align:start}button{appearance:none}html{background:white}body{display:flow-root}";
    await context.addInitScript(() => {
        Object.defineProperty(Node.prototype, "setText", { value(value) { this.textContent = String(value); } });
        const add = EventTarget.prototype.addEventListener;
        Object.defineProperty(EventTarget.prototype, "addEventListener", { value(type, listener, options) {
            if (!options || typeof options !== "object" || !options.scope) return add.call(this, type, listener, options);
            const adapted = Object.assign({}, options, { signal: options.scope.signal });
            delete adapted.scope;
            return add.call(this, type, listener, adapted);
        }});
        globalThis.events = { scope() {
            const controller = new AbortController();
            return { signal: controller.signal, dispose() { controller.abort(); } };
        }};
        globalThis.shortcuts = { register() { return { dispose() {} }; } };
        globalThis.bridge = { postMessage() {}, onMessage() {} };
        Object.defineProperty(HTMLInputElement.prototype, "getValue", { value() { return this.value; } });
        Object.defineProperty(HTMLInputElement.prototype, "setValue", { value(value) { this.value = String(value); } });
        Object.defineProperty(Symbol, "plectron", { value: Symbol("plectron") });
        Object.defineProperty(Symbol, Symbol.plectron, { value: Object.freeze({
            document, events: globalThis.events, shortcuts: globalThis.shortcuts, bridge: globalThis.bridge
        }) });
    });
    await target.route("**/" + config.scenario + "/index.html", async route => {
        const response = await route.fetch();
        const source = await response.text();
        await route.fulfill({ response, body: "<!doctype html>\n" + source.replace("<head>", "<head><style>" + reset + "</style>").replace(/<script\b[^>]*src=[^>]*>\s*<\/script>/g, "") });
    });
    await target.route("**/" + config.scenario + "/style.css", async route => {
        const response = await route.fetch();
        await route.fulfill({ response, body: await response.text() + "\n" + config.stylesheet });
    });
    const cdp = await context.newCDPSession(target);
    await cdp.send("Performance.enable", { timeDomain: "timeTicks" });
    const metrics = async () => Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map(item => [item.name, item.value]));
    const host = await browser.newBrowserCDPSession();
    const report = { version: browser.version(), config, browser: await host.send("Browser.getVersion"),
        gpu: (await host.send("SystemInfo.getInfo")).gpu, reset, errors, runs: [],
        endpoint: "adapter invocation of mutation to encoded PNG available in adapter; includes CDP transport and capture synchronization/encoding; excludes disk write and assertions",
        classification: "capture API observation; not native GPU execution or physical presentation",
        clock: "adapter Date.now wall clock, millisecond resolution",
        metrics: { timeDomain: "timeTicks", snapshots: "raw cumulative CDP metrics before and after each action; duration values are seconds, counts retain native units" },
        accounting: "Performance metric deltas bracket mutation and timed screenshot; exclude settled-oracle verification. TaskDuration includes main-thread protocol work, excludes GPU execution; metrics are nested, not additive.",
        adaptations: ["standards doctype", "explicit ordered workload function scope after DOM loading", "setText/textContent", "scoped listeners/AbortSignal", "platform addon stubs", "frozen engine host registry", "input value adapters", "Rust-translated script response receipts", "scroll frame synchronization", "capture caret suppression", "explicit UA reset", "shared normalization stylesheet"],
    };
    const actions = ["seed", "steady", ...config.actions];
    const seeded = Array.from({ length: config.count }, (_, i) => i);
    const appended = Array.from({ length: Math.ceil(config.count / 4) }, (_, i) => config.count + i);
    const mutate = ({ action, seeded, appended, region }) => {
        const steps = {
            seed: () => scenario.seed(seeded),
            append: () => scenario.run(action, appended),
            scroll: () => { document.getElementById(region).scrollTop += 48; },
            steady: () => {},
        };
        (steps[action] ?? (() => scenario.run(action, [0])))();
    };
    const inspect = config => {
        const region = config.region;
        const root = document.getElementById(region);
        const rect = node => { const r = node.getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; };
        const boxes = {region: rect(root)};
        [...root.children].slice(0, 3).forEach((node, index) => boxes["record-" + index] = rect(node));
        const records = config.regions.reduce((count, id) => count + document.getElementById(id).children.length, 0);
        const first = config.scenario === "desk" ? rows[0].node : root.firstElementChild;
        return { records, first: first.id, label: first.firstChild.textContent,
            status: document.getElementById("status").textContent, geometry: { boxes, scroll: root.scrollTop, extent: root.scrollHeight },
            fonts: Array.from(document.fonts, font => ({family: font.family, status: font.status})) };
    };
    const run = async (mode, index) => {
        await target.goto(config.url + config.scenario + "/index.html");
        await target.evaluate(async config => {
            const bodies = [];
            for (const path of config.order) {
                const response = await fetch(config.url + path);
                if (!response.ok) throw new Error("workload script failed: " + path);
                bodies.push(await response.text());
            }
            const workload = new Function(bodies.join("\n") + "\nreturn { scenario, rows: () => typeof rows === 'undefined' ? [] : rows };")();
            globalThis.scenario = workload.scenario;
            Object.defineProperty(globalThis, "rows", { get: workload.rows });
        }, config);
        await target.evaluate(async () => { await Promise.all(Array.from(document.fonts, font => font.load())); await document.fonts.ready; });
        const stages = [];
        for (const action of actions) {
            const before = await metrics();
            const start = Date.now();
            await target.evaluate(mutate, { action, seeded, appended, region: config.region });
            if (action === "scroll") await target.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
            const dispatched = Date.now();
            const png = (await cdp.send("Page.captureScreenshot", { format: "png", fromSurface: true, captureBeyondViewport: false })).data;
            const ready = Date.now() - start;
            const after = await metrics();
            const work = Object.fromEntries(["TaskDuration", "ScriptDuration", "LayoutDuration", "RecalcStyleDuration"].map(name => [name, (after[name] - before[name]) * 1000]));
            if (ready < 0) throw new Error("adapter clock moved backwards");
            await target.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
            const oracle = (await cdp.send("Page.captureScreenshot", { format: "png", fromSurface: true, captureBeyondViewport: false })).data;
            const state = await target.evaluate(inspect, config);
            if (config.scenario === "article" && action === "seed" && !state.label.includes("café é")) throw new Error("article source is not UTF-8");
            const expanded = config.count + Math.ceil(config.count / 4);
            const expected = ["seed", "steady"].includes(action) ? config.count : expanded - (actions.indexOf(action) >= actions.indexOf("remove") ? 1 : 0);
            if (state.records !== expected || !state.status.endsWith(": " + expected)) throw new Error("record mismatch: " + action);
            if (action === "edit" && !state.label.includes("Changed ")) throw new Error("edit not applied");
            if (action === "reorder" && state.first !== config.prefix + "1") throw new Error("reorder not applied");
            if (action === "remove" && state.first !== config.prefix + "2") throw new Error("remove not applied");
            if (action === "scroll" && state.geometry.scroll !== 48) throw new Error("scroll not applied");
            if (!state.fonts.some(font => font.family === "lato" && font.status === "loaded")) throw new Error("font missing");
            stages.push({ action, ready, dispatch: dispatched - start, capture: ready - (dispatched - start), metrics: { before, after }, work, state, png, oracle });
        }
        return { mode, index, stages };
    };
    try {
        for (let index = 0; index < config.warmups; index++) await run("warmup", index);
        for (let index = 0; index < config.runs; index++) report.runs.push(await run("capture", index));
        report.requests = await target.evaluate(() => Array.from(new Set(performance.getEntriesByType("resource").map(entry => new URL(entry.name).pathname.slice(1)))));
        report.scripts = await Promise.all(receipts);
        if (errors.length) throw new Error(errors.join("\n"));
        return report;
    } finally {
        await host.detach();
        await context.close();
    }
}
