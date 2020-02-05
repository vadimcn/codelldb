import { ViewColumn } from "vscode";

export class AdapterSettings {
    showDisassembly: 'always' | 'auto' | 'never' = 'auto';
    displayFormat: 'auto' | 'hex' | 'decimal' | 'binary' = 'auto';
    dereferencePointers: boolean = true;
    evaluationTimeout: number;
    suppressMissingSourceFiles: boolean;
    consoleMode: 'commands' | 'expressions';
    sourceLanguages: string[];
    defaultPanicBreakpoint: boolean = true;
    defaultCatchBreakpoint: boolean = false;
};

export interface DisplayHtmlRequest {
    title: string;
    position: ViewColumn;
    html: string;
    reveal: boolean;
}
