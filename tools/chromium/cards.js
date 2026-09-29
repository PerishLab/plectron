async page => {
    const config = __CONFIG__;
    const browser = page.context().browser();
    const context = await browser.newContext({ viewport: { width: 720, height: 640 }, deviceScaleFactor: 1 });
    const target = await context.newPage();
    const errors = [];
    target.on("pageerror", error => errors.push(String(error)));
    const reset = "*{margin:0;padding:0;border:0;box-sizing:content-box}h1,button{font:inherit;text-align:start}button{appearance:none}html{background:white}body{display:flow-root}";
    await context.addInitScript(() => {
        Object.defineProperty(Node.prototype, "setText", {
            value(value) { this.textContent = String(value); },
        });
    });
    await target.route("**/cards/index.html", async route => {
        const response = await route.fetch();
        const source = await response.text();
        const body = "<!doctype html>\n" + source.replace("<head>", "<head><style>" + reset + "</style>")
            .replace("<script src=", "<script defer src=");
        await route.fulfill({ response, body });
    });
    const cdp = await context.newCDPSession(target);
    const host = await browser.newBrowserCDPSession();
    const system = await host.send("SystemInfo.getInfo");
    const version = await host.send("Browser.getVersion");
    await cdp.send("Performance.enable", { timeDomain: "timeTicks" });
    const metrics = async () => Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map(m => [m.name, m.value]));
    const report = {
        version: browser.version(), browser: version, config, reset,
        gpu: system.gpu, errors, runs: [],
        adaptations: ["standards doctype", "deferred entry script", "Node.setText maps to textContent", "explicit UA reset recorded in reset"],
        differences: ["Chromium still collapses sibling margins", "scroll uses scrollTop, not a wheel event", "headless rendering without physical presentation"],
        endpoint: "two animation-frame callbacks after mutation; a rendering opportunity, not GPU completion or physical presentation",
        accounting: "CDP duration deltas in milliseconds; TaskDuration includes harness and protocol work; trace intervals may overlap across threads",
    };
    const actions = ["seed", "steady", "append", "edit", "reorder", "remove", "width", "scroll"];
    const run = async (mode, index) => {
        await target.goto(config.url + "cards/index.html");
        await target.evaluate(async () => {
            await document.fonts.load("14px lato");
            await document.fonts.ready;
        });
        const stages = [];
        for (const action of actions) {
            const before = await metrics();
            const marker = ["plectron", mode, index, action].join(":");
            const observed = await target.evaluate(({ action, count, marker }) => new Promise(resolve => {
                requestAnimationFrame(() => {
                    performance.mark(marker + ":start");
                    const start = performance.now();
                    if (action === "seed") scenario.seed(Array.from({ length: count }, (_, i) => i));
                    else if (action === "append") scenario.run(action, Array.from({ length: Math.ceil(count / 4) }, (_, i) => count + i));
                    else if (action === "scroll") document.getElementById("cards").scrollTop += 48;
                    else if (action !== "steady") scenario.run(action, [0]);
                    const script = performance.now() - start;
                    performance.mark(marker + ":script");
                    requestAnimationFrame(() => requestAnimationFrame(() => {
                        performance.mark(marker + ":end");
                        resolve({ script, window: performance.now() - start });
                    }));
                });
            }), { action, count: config.count, marker });
            const after = await metrics();
            const timing = {};
            for (const name of ["TaskDuration", "ScriptDuration", "RecalcStyleDuration", "LayoutDuration"]) {
                timing[name] = (after[name] - before[name]) * 1000;
            }
            const state = await target.evaluate(() => {
                const root = document.getElementById("cards");
                const first = root.firstElementChild;
                const walker = document.createTreeWalker(document);
                let nodes = 1;
                while (walker.nextNode()) nodes++;
                return {
                    records: root.children.length, first: first.id, label: first.firstChild.textContent,
                    status: document.getElementById("status").textContent,
                    region: root.getBoundingClientRect().toJSON(), card: first.getBoundingClientRect().toJSON(),
                    scroll: root.scrollTop, extent: root.scrollHeight,
                    elements: document.querySelectorAll("*").length, nodes,
                    fonts: Array.from(document.fonts, f => ({ family: f.family, status: f.status })),
                    dpr: devicePixelRatio, standards: document.compatMode,
                };
            });
            const expanded = config.count + Math.ceil(config.count / 4);
            const expected = ["seed", "steady"].includes(action) ? config.count : expanded - (["remove", "width", "scroll"].includes(action) ? 1 : 0);
            if (state.records !== expected || state.status !== "Records: " + expected) throw new Error("record mismatch: " + action);
            if (action === "edit" && state.label !== "Changed Card 0") throw new Error("edit was not applied");
            if (action === "reorder" && state.first !== "item-1") throw new Error("reorder was not applied");
            if (action === "remove" && state.first !== "item-2") throw new Error("remove was not applied");
            if (action === "width" && state.region.width !== 440) throw new Error("width was not applied");
            if (action === "scroll" && state.scroll !== 48) throw new Error("scroll was not applied");
            if (state.standards !== "CSS1Compat" || state.dpr !== 1 || !state.fonts.some(f => f.family === "lato" && f.status === "loaded")) throw new Error("rendering environment mismatch");
            stages.push({ action, marker, timing, observed, state,
                layouts: after.LayoutCount - before.LayoutCount,
                styles: after.RecalcStyleCount - before.RecalcStyleCount });
            if (mode === "visual" && ["append", "edit", "width", "scroll"].includes(action)) {
                await target.screenshot({ path: config.output + "/" + action + ".png" });
            }
        }
        return { mode, index, stages };
    };
    try {
        report.visual = await run("visual", 0);
        for (let index = 0; index < config.warmups; index++) await run("warmup", index);
        for (let index = 0; index < config.runs; index++) report.runs.push(await run("plain", index));
        await browser.startTracing(target, {
            path: config.output + "/trace.json", screenshots: false,
            categories: ["devtools.timeline", "blink.user_timing", "toplevel", "cc", "gpu", "viz", "disabled-by-default-devtools.timeline"],
        });
        try {
            for (let index = 0; index < config.runs; index++) report.runs.push(await run("trace", index));
        } finally {
            await browser.stopTracing();
        }
        if (errors.length) throw new Error(errors.join("\n"));
        return report;
    } finally {
        await host.detach();
        await context.close();
    }
}
