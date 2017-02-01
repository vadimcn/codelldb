'use strict';
import {ProtocolClient} from './protocolClient';
import {window} from 'vscode';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import * as net from 'net';

export class MyProtocolClient extends ProtocolClient {

    public isActive = false;

    start(port: number): Promise<void> {
        return new Promise<void>((resolve, reject) => {
            let conn = net.connect(port, '127.0.0.1', () => {
                this.connect(conn, conn);
                this.isActive = true;
                resolve();
            });
            conn.on('error', (err:any) => {
                reject();
                this.isActive = false;
            });
            conn.on('end', () => {
                this.isActive = false;
            })
        });
    }
}

var connection: MyProtocolClient = null;

export async function withSession<T>(operation: (conn: MyProtocolClient) => Promise<T>): Promise<T> {
    if (!connection || !connection.isActive) {
        connection = await createConnection();
    }
    try {
        return await operation(connection);
    } catch (e) {
        await window.showErrorMessage('Could not send command: ' + e.message);
        throw e;
    }
}

async function createConnection(): Promise<MyProtocolClient> {
    try {
        let extInfoPath = path.join(os.tmpdir(), 'vscode-lldb-session-' + process.env['VSCODE_PID']);
        let data = fs.readFileSync(extInfoPath, 'utf8');
        let port = parseInt(data);
        let client = new MyProtocolClient();
        await client.start(port);
        return client;
    } catch (err) {
        await window.showErrorMessage('Could not establish connection to debug adapter: ' + err.message);
        throw err;
    }
}
