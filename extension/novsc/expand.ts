let expandVarRegex = /\$\{(?:([^:}]+):)?([^}]+)\}/g;

export function expandVariables(str: string | String, expander: (type: string, key: string) => string): string {
    let result = str.replace(expandVarRegex, (all: string, type: string, key: string): string => {
        let replacement = expander(type, key);
        return replacement != null ? replacement : all;
    });
    return result;
}

export function expandVariablesInObject(obj: any, expander: (type: string, key: string) => string): any {
    if (typeof obj == 'string' || obj instanceof String)
        return expandVariables(obj, expander);

    if (isScalarValue(obj))
        return obj;

    if (obj instanceof Array)
        return obj.map(v => expandVariablesInObject(v, expander));

    for (let prop of Object.keys(obj))
        obj[prop] = expandVariablesInObject(obj[prop], expander)
    return obj;
}

function isScalarValue(value: any): boolean {
    return value === null || value === undefined ||
        typeof value == 'boolean' || value instanceof Boolean ||
        typeof value == 'number' || value instanceof Number ||
        typeof value == 'string' || value instanceof String;
}


export function mergeValues(value1: any, value2: any): any {
    if (value2 === undefined)
        return value1;
    // For non-container types, value2 wins.
    if (isScalarValue(value1))
        return value2;
    // Concatenate arrays.
    if (value1 instanceof Array && value2 instanceof Array)
        return value1.concat(value2);
    // Merge dictionaries.
    return Object.assign({}, value1, value2);
}
