import { ViewColumn } from "vscode";

export class AdapterSettings {
    showDisassembly: 'always' | 'auto' | 'never' = 'auto';
    displayFormat: 'auto' | 'hex' | 'decimal' | 'binary' = 'auto';
    dereferencePointers: boolean = true;
    evaluationTimeout: number;
    suppressMissingSourceFiles: boolean;
    consoleMode: 'commands' | 'evaluate';
    sourceLanguages: string[];
    terminalPromptClear: string[];
    evaluateForHovers: boolean;
    commandCompletions: boolean;
    reproducer: boolean | string;
};

export interface DisplayHtmlRequest {
    title: string;
    position: ViewColumn;
    html: string;
    reveal: boolean;
}

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
