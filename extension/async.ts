import * as _fs from 'fs';
import * as _cp from 'child_process';
import * as _http from 'http';
import * as _https from 'https';
import { promisify } from 'util';
import { Uri } from 'vscode';

export namespace fs {
    export const readdir = promisify(_fs.readdir);
    export const readFile = promisify(_fs.readFile);
    export const exists = promisify(_fs.exists);
    export const stat = promisify(_fs.stat);
}

export namespace cp {
    export const execFile = promisify(_cp.execFile);
}

export namespace https {
    export function get(url: string | URL | Uri): Promise<_http.IncomingMessage> {
        if (url instanceof Uri)
            url = url.toString(true);
        return new Promise<_http.IncomingMessage>((resolve, reject) => {
            try {
                let request = _https.get(url, resolve);
                request.on('error', err => reject(err));
            } catch (err) {
                reject(err);
            }
        });
    }
}
