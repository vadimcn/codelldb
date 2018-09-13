export interface Dict<T> {
    [key: string]: T;
}

export type AdapterType = 'classic' | 'bundled' | 'native';

export function toAdapterType(str: string): AdapterType {
    return str == 'bundled' ? 'bundled' : (str == 'native' ? 'native' : 'classic');
}
