'use strict';
import {ProtocolClient} from './protocolClient';
import {window} from 'vscode';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import * as net from 'net';

export class MyProtocolClient extends ProtocolClient {

    public isActive = false;

    constructor(conn: net.Socket) {
        super();
        super.connect(conn, conn);
        this.isActive = true;
        conn.on('error', (err:any) => {
            this.isActive = false;
        });
        conn.on('end', () => {
            this.isActive = false;
        })
    }
}

var server: net.Server = null;
var connection: MyProtocolClient = null;

export async function startListener(): Promise<number> {
    return new Promise<number>((resolve, reject) => {
        if (!server) {
            server = net.createServer((conn) => {
                connection = new MyProtocolClient(conn);
            });
            server.listen(() => {
                resolve(server.address().port);
            })
        } else {
            resolve(server.address().port);
        }
    });
}

export async function execute<T>(operation: (conn: MyProtocolClient) => Promise<T>): Promise<T> {
    if (!connection || !connection.isActive) {
        await window.showErrorMessage('No connection to the debug session.');
        throw new Error('No connection to the debug session.');
    }
    try {
        return await operation(connection);
    } catch (e) {
        await window.showErrorMessage('Could not send command: ' + e.message);
        throw e;
    }
}
