import { Disposable } from "vscode";

export interface Dict<T> {
    [key: string]: T;
}

export class MapEx<K, V> extends Map<K, V> {
    setdefault(key: K, def: V | (() => V)): V {
        let value = super.get(key);
        if (value == undefined) {
            value = def instanceof Function ? def() : def;
            super.set(key, value);
        }
        return value;
    }
}

export class DisposableSubscriber extends Disposable {
    public readonly subscriptions: Disposable[] = [];

    constructor() {
        super(() => this.subscriptions.forEach(item => item.dispose()));
    }
}
