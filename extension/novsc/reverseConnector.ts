import { DebugAdapter, DebugAdapterInlineImplementation, DebugProtocolMessage, Event, EventEmitter } from 'vscode';
import * as net from 'net';
import { WritableBuffer } from './writableBuffer';

/// Allows debug adapter to reverse-connect to VSCode
export class ReverseAdapterConnector implements DebugAdapter {
    private server: net.Server = net.createServer();
    private connection: net.Socket;
    private rawData: WritableBuffer = new WritableBuffer();
    private contentLength: number = -1;
    private onDidSendMessageEmitter = new EventEmitter<DebugProtocolMessage>();

    constructor() {
        this.onDidSendMessage = this.onDidSendMessageEmitter.event;
    }

    async listen(port: number = 0): Promise<number> {
        return new Promise(resolve => {
            this.server.listen(port, '127.0.0.1', () => {
                let address = <net.AddressInfo>this.server.address();
                resolve(address.port);
            });
        });
    }

    async accept(): Promise<void> {
        return new Promise(resolve => {
            this.server.on('connection', socket => {
                this.connection = socket;
                socket.on('data', data => this.handleData(data));
                resolve();
            })
        });
    }

    readonly onDidSendMessage: Event<DebugProtocolMessage>;

    handleMessage(message: DebugProtocolMessage): void {
        let json = JSON.stringify(message);
        this.connection.write(`Content-Length: ${Buffer.byteLength(json, 'utf8')}\r\n\r\n${json}`, 'utf8');
    }

    private handleData(data: Buffer): void {
        this.rawData.write(data);
        while (true) {
            if (this.contentLength >= 0) {
                if (this.rawData.length >= this.contentLength) {
                    let message = this.rawData.head(this.contentLength);
                    if (message.length > 0) {
                        try {
                            let msg: DebugProtocolMessage = JSON.parse(message.toString('utf8'));
                            this.onDidSendMessageEmitter.fire(msg);
                        }
                        catch (e) {
                            console.log('Error handling data: ' + (e && e.message));
                        }
                    }
                    this.rawData.remove(this.contentLength);
                    this.contentLength = -1;
                    continue;	// there may be more complete messages to process
                }
            } else {
                let idx = this.rawData.contents.indexOf('\r\n\r\n')
                if (idx !== -1) {
                    let header = this.rawData.head(idx).toString('utf8');
                    let lines = header.split('\r\n');
                    for (let i = 0; i < lines.length; i++) {
                        const pair = lines[i].split(/: +/);
                        if (pair[0] == 'Content-Length') {
                            this.contentLength = +pair[1];
                        }
                    }
                    this.rawData.remove(idx + 4);
                    continue;
                }
            }
            break;
        }
    }

    dispose() {
        if (this.connection)
            this.connection.destroy();
        this.server.close();
    }
}
