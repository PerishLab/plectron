type Registry = typeof Symbol & Record<"plectron", symbol>;
const read = (Symbol: Registry) => Symbol[Symbol.plectron];
const host = read(Symbol as Registry);

export const platform = host.platform;
export const console = host.console;
export const document = host.document;
export const events = host.events;
export const scroll = host.scroll;
export const visual = host.visual;
export const bridge = host.bridge;
export const shortcuts = host.shortcuts;
export const fetch = host.fetch;
export const requests = host.requests;
export const surface = host.surface;
