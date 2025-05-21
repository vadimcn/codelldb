
export class AdapterSettings {
    showDisassembly?: 'always' | 'auto' | 'never' = 'auto';
    displayFormat?: 'auto' | 'hex' | 'decimal' | 'binary' = 'auto';
    dereferencePointers?: boolean = true;
    evaluationTimeout?: number;
    summaryTimeout?: number;
    suppressMissingSourceFiles?: boolean;
    consoleMode?: 'commands' | 'evaluate';
    sourceLanguages?: string[];
    scriptConfig?: object;
    evaluateForHovers?: boolean;
    commandCompletions?: boolean;
    reproducer?: boolean | string;
};

export class Symbol {
    name: string;
    type: string;
    address: string;
    location: any
}

export type SymbolsRequest = {
    filter: string,
    maxResults: number
}

export type SymbolsResponse = {
    symbols: Symbol[];
}

export type ExcludeCallerRequest = {
    threadId: number;
    frameIndex: number;
}

export type ExcludeCallerResponse = {
    breakpointId: number | [string, string];
    symbol: string
}

export type SetExcludedCallersRequest = {
    exclusions: {
        breakpointId: string | number;
        symbol: string
    }[];
}
