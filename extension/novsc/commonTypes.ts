export type AdapterType = 'classic' | 'native';

export function toAdapterType(str: string): AdapterType {
    return str == 'classic' ? 'classic' : 'native';
}

export interface Dict<T> {
    [key: string]: T;
}

// Windows environment varibles are case-insensitive: for example, `Path` and `PATH` refer to the same variable.
// This class emulates such a behavior.
export class Environment {
    constructor(ignoreCase: boolean) {
        if (ignoreCase)
            return new Proxy(this, new IgnoreCaseProxy());
        else
            return this;
    }
    [key: string]: string;
}

class IgnoreCaseProxy {
    private keys: Dict<string> = {};

    get(target: any, key: string) {
        let upperKey = key.toUpperCase();
        let mappedKey = this.keys[upperKey];
        return target[mappedKey];
    }

    set(target: any, key: string, value: any): boolean {
        let upperKey = key.toUpperCase();
        let mappedKey = this.keys[upperKey];
        if (mappedKey == undefined) {
            this.keys[upperKey] = key;
            mappedKey = key;
        }
        target[mappedKey] = value;
        return true;
    }
}
