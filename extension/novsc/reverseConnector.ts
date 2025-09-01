import { DebugAdapter, DebugAdapterInlineImplementation, DebugProtocolMessage, Event, EventEmitter } from 'vscode';
import * as net from 'net';
import { WritableBuffer } from './writableBuffer';

/// Allows debug adapter to reverse-connect to VSCode
export class ReverseAdapterConnector implements DebugAdapter {
    private server: net.Server = net.createServer();
    private connection?: net.Socket;
    private rawData: WritableBuffer = new WritableBuffer();
    private contentLength: number = -1;
    private authToken: string;
    private authenticated: boolean = false;
    private onDidSendMessageEmitter = new EventEmitter<DebugProtocolMessage>();
    readonly onDidSendMessage: Event<DebugProtocolMessage>;

    constructor(authToken: string) {
        this.authToken = authToken;
        this.onDidSendMessage = this.onDidSendMessageEmitter.event;
    }

    async listen(port: number = 0): Promise<number> {
        return new Promise(resolve => {
            this.server.listen(port, '127.0.0.1', () => {
                let address = this.server.address() as net.AddressInfo;
                resolve(address.port);
            });
        });
    }

    async accept(): Promise<void> {
        return new Promise(resolve => {
            this.server.on('connection', socket => {
                this.server.close();
                this.connection = socket;
                socket.on('data', data => this.handleData(data));
                resolve();
            })
        });
    }

    handleMessage(message: DebugProtocolMessage): void {
        let json = JSON.stringify(message);
        this.connection!.write(`Content-Length: ${Buffer.byteLength(json, 'utf8')}\r\n\r\n${json}`, 'utf8');
    }

    private handleData(data: Buffer): void {
        this.rawData.write(data);
        while (true) {
            if (this.contentLength < 0) {
                // Wait till we have received all headers
                let idx = this.rawData.contents.indexOf('\r\n\r\n')
                if (idx != -1) {
                    let header = this.rawData.head(idx).toString('utf8');
                    let lines = header.split('\r\n');
                    for (let i = 0; i < lines.length; i++) {
                        let pair = lines[i].split(/: +/, 2);
                        if (pair[0] == 'Content-Length') {
                            this.contentLength = parseInt(pair[1]);
                        } else if (pair[0] == 'Auth-Token') {
                            if (pair[1] == this.authToken) {
                                this.authenticated = true;
                            }
                        }
                    }
                    this.rawData.remove(idx + 4);
                    continue;
                }
            } else {
                // Shouldn't be here unless authenticated
                if (!this.authenticated) {
                    this.dispose();
                    return;
                }

                if (this.rawData.length >= this.contentLength) {
                    let message = this.rawData.head(this.contentLength);
                    if (message.length > 0) {
                        try {
                            let msg = JSON.parse(message.toString('utf8')) as DebugProtocolMessage;
                            this.onDidSendMessageEmitter.fire(msg);
                        }
                        catch (err: any) {
                            console.log('Error handling data: ' + err.toString());
                        }
                    }
                    this.rawData.remove(this.contentLength);
                    this.contentLength = -1;
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
