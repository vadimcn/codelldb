
export class AdapterSettings {
    showDisassembly?: 'always' | 'auto' | 'never' = 'auto';
    displayFormat?: 'auto' | 'hex' | 'decimal' | 'binary' = 'auto';
    dereferencePointers?: boolean = true;
    evaluationTimeout?: number;
    suppressMissingSourceFiles?: boolean;
    consoleMode?: 'commands' | 'evaluate';
    sourceLanguages?: string[];
    terminalPromptClear?: string[];
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

export interface SymbolsRequest {
    filter: string,
    maxResults: number
}

export interface SymbolsResponse {
    symbols: Symbol[];
}

export interface ExcludeCallerRequest {
    source: number | string;
    line: number;
    column: number;
}

export interface ExcludeCallerResponse {
    breakpointId: number | [string, string] ;
    symbol: string
}


export interface SetExcludedCallersRequest {
    exclusions: {
        breakpointId: string | number;
        symbol: string
    }[];
}
