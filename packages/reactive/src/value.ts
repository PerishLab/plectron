export const same = (left: unknown, right: unknown) => {
    if (left === right) { return left !== 0 || 1 / left === 1 / (right as number); }
    return left !== left && right !== right;
};

export const remove = <T>(items: T[], value: T) => {
    let found = false;
    let index = 0;
    for (const item of items) {
        if (!found && item === value) { found = true; continue; }
        if (found) { items[index] = item; }
        index += 1;
    }
    if (found) { items.length -= 1; }
};
