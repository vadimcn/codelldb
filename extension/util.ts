'use strict';

let expandVarRegex = new RegExp('\\$\\{(?:([^:}]+):)?([^}]+)\\}', 'g');

export function expandVariables(str: string, expander: (type: string, key: string) => string): string {
    let result = str.replace(expandVarRegex, (all: string, type: string, key: string): string => {
        return expander(type, key);
    });
    return result;
}
