const record = (failure, secondary, phase, terminal) => ({
    primary: failure.primary,
    secondary: secondary,
    terminal: terminal,
    phase: phase,
    generation: failure.generation,
    owner: failure.owner
});

export default function Boundary(context) {
    const document = context.document;
    const host = context.host;
    const content = context.content;
    const failed = context.failed;
    const report = context.report;
    let current = null;
    let disposed = false;

    const notify = (failure) => {
        if (report === undefined) { return; }
        try { report(failure); }
        catch (error) { Promise.reject(error); }
    };
    const release = (instance) => {
        if (instance !== null && typeof instance.dispose === "function") {
            instance.dispose();
        }
    };
    const commit = (instance) => {
        const previous = current;
        host.appendChild(instance.host);
        if (previous !== null) { host.removeChild(previous.host); }
        current = instance;
        release(previous);
    };
    const terminal = () => {
        const node = document.createElement("div");
        node.setAttribute("role", "alert");
        node.setAttribute("data-plectron-terminal", "");
        node.setText("Content failed");
        return {host: node};
    };
    const finish = (failure, cause) => {
        try {
            commit(terminal());
            if (cause === undefined) { notify(failure); }
            else { notify(record(failure, cause, "failed")); }
        } catch (error) {
            notify(record(failure, cause, "terminal", error));
            Promise.reject(error);
        }
    };
    const recover = (error, reset, failure) => {
        if (failed === undefined) {
            finish(failure, undefined);
            return;
        }
        try {
            const instance = failed({
                document: document, error: error, reset: reset,
                generation: failure.generation
            });
            commit(instance);
            notify(failure);
        } catch (next) { finish(failure, next); }
    };
    const region = context.scope.boundary((wrap) => {
        const instance = content({document: document, wrap: wrap});
        commit(instance);
        return instance;
    }, recover);
    const dispose = () => {
        if (disposed) { return; }
        disposed = true;
        region.dispose();
        release(current);
        current = null;
    };
    return {
        host: host,
        reset: region.reset,
        dispose: dispose,
        get generation() { return region.generation; },
        get failed() { return region.failed; }
    };
};
