
export interface Dict<T> {
    [key: string]: T;
}

// Windows environment varibles are case-insensitive: for example, `Path` and `PATH` refer to the same variable.
// This class emulates such a behavior.
export class Environment {
    constructor(ignoreCase: boolean = (process.platform == 'win32')) {
        if (ignoreCase)
            return new Proxy(this, new IgnoreCaseProxy());
        else
            return this;
    }
    [key: string]: string;
}

class IgnoreCaseProxy {
    private keys: Dict<string> = {};

    get(target: any, key: string): any {
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

    deleteProperty(target: any, key: string): boolean {
        let upperKey = key.toUpperCase();
        let mappedKey = this.keys[upperKey];
        delete target[mappedKey];
        return true;
    }
}
