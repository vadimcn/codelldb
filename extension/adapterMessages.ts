import { ViewColumn } from "vscode";

export class DisplaySettings {
    showDisassembly: 'always' | 'auto' | 'never' = 'auto';
    displayFormat: 'auto' | 'hex' | 'decimal' | 'binary' = 'auto';
    dereferencePointers: boolean = true;
    containerSummary: boolean = true;
};

export interface DisplayHtmlRequest {
    title: string;
    position: ViewColumn;
    html: string;
    reveal: boolean;
}
