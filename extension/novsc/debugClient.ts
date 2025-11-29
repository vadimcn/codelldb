import { DebugClient as _DebugClient } from '@vscode/debugadapter-testsupport';
import * as stream from 'node:stream';

export class DebugClient extends _DebugClient {
    constructor(debugType: string) {
        super('', '', debugType);
    }
    connect(readable: stream.Readable, writable: stream.Writable) {
        super.connect(readable, writable);
    }
}
