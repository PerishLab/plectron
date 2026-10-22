export type State = { count: number };

export const create = function (): State {
    return { count: 0 };
};

export const fail = function (): never {
    throw "typescript listener";
};

export const later = async function (): Promise<never> {
    await 1;
    throw "typescript async";
};
