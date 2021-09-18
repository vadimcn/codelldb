import { ViewColumn } from "vscode";

export class AdapterSettings {
    showDisassembly: 'always' | 'auto' | 'never' = 'auto';
    displayFormat: 'auto' | 'hex' | 'decimal' | 'binary' = 'auto';
    dereferencePointers: boolean = true;
    evaluationTimeout: number;
    suppressMissingSourceFiles: boolean;
    consoleMode: 'commands' | 'expressions';
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
}

export interface SymbolsRequest {
    continuationToken: object;
}

export interface SymbolsResponse {
    symbols: Symbol[];
    continuationToken: object;
}
