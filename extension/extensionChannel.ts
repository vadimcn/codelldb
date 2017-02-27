'use strict';
import {ProtocolClient} from './protocolClient';
import {window} from 'vscode';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import * as net from 'net';

export class MyProtocolClient extends ProtocolClient {

    public isActive = false;

    public connect(conn: net.Socket) {
        super.connect(conn, <any>conn);
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
var connection: MyProtocolClient = null; new MyProtocolClient();

export async function startListener(): Promise<number> {
    return new Promise<number>((resolve, reject) => {
        connection = new MyProtocolClient();
        if (!server) {
            server = net.createServer((conn) => {
                connection.connect(conn);
            });
            server.listen(() => {
                resolve(server.address().port);
            })
        } else {
            resolve(server.address().port);
        }
    });
}

export function isActive() {
    return connection.isActive;
}

export function channel(): ProtocolClient {
    return connection;
}

export async function execute<T>(operation: (conn: ProtocolClient) => Promise<T>): Promise<T> {
    return await operation(channel());
}
