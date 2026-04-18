import { Dict } from "./commonTypes";

// Returning null means "keep the original text".
type Expander = (type: string | null, key: string) => string | null;

let expandVarRegex = /\$\{(?:([^:}]+):)?([^}]+)\}/g;

export function expandVariables<T extends any>(obj: T, expander: Expander): T {

    if (typeof obj == 'string' || obj instanceof String) {
        return obj.replace(expandVarRegex, (all: string, type: string, key: string): string => {
            let replacement = expander(type, key);
            return replacement != null ? replacement : all;
        }) as T;
    }

    if (isScalarValue(obj))
        return obj;

    if (obj instanceof Array)
        return obj.map(v => expandVariables(v, expander)) as T;

    let result: Dict<any> = {};
    for (let prop of Object.getOwnPropertyNames(obj))
        result[prop] = expandVariables((obj as Dict<any>)[prop], expander);
    return result as T;
}

function isScalarValue(value: any): boolean {
    return value === null || value === undefined ||
        typeof value == 'boolean' || value instanceof Boolean ||
        typeof value == 'number' || value instanceof Number ||
        typeof value == 'string' || value instanceof String;
}

// In conflicts, value1 wins.
export function mergeValues(value1: any, value2: any, reverseSeq: boolean = false): any {
    if (value1 === undefined) {
        return value2;
    } else if (value2 === undefined) {
        return value1;
    } else if (isScalarValue(value1) || isScalarValue(value2)) {
        return value1;
    } else if (value1 instanceof Array && value2 instanceof Array) {
        if (!reverseSeq)
            return value1.concat(value2);
        else
            return value2.concat(value1)
    } else {
        return Object.assign({}, value2, value1);
    }
}
