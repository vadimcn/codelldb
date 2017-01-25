// Adapter launcher which takes care of spawning the LLDB process and monitoring it for errors.
'use strict';

import * as cp from 'child_process';
import * as os from 'os';
import * as path from 'path';
import { Readable, Writable } from 'stream';

function main() {
    let extInfoPath = path.join(os.tmpdir(), 'vscode-lldb-session').replace(/\\/g, '\\\\');
    let launchScript = 'script import adapter\r\n' +
        'script adapter.run_stdio_session(3,4,extinfo=\'' + extInfoPath + '\')\r\n' +
        'exit\r\n';

    // Spawn LLDB.  Stdio streams 3 and 4 will be connected to our stdin and stdout.
    // Unfortunately, we cannot hand off our stdin to LLDB directly, because Node.js 
    // will have already read the initial commands that VSCode sends into a buffer 
    // in this process, so instead we just pipe the data.
    let lldb = cp.spawn('lldb', [], {
        stdio: ['pipe', 'pipe', 'ignore', 'pipe', process.stdout],
        cwd: __dirname + '/..',
        //env: { 'VSCODE_LLDB_LOG': '/tmp/vscode-lldb.log' }
    });
    process.stdin.pipe(<Writable>lldb.stdio[3]);

    // In case there are problems with launching...
    lldb.on('error', (err: any) => {
        send_error_msg('Failed to launch LLDB: ' + err.message, err.message);
        process.exit(1);
    });

    // Send the adapter launch script.
    if (lldb.pid) { // (write() will throw on Windows if spawn fails)
        lldb.stdin.write(launchScript);
    }

    // Monitor LLDB output for traceback spew and send it to debug console.
    // This is about the only way to catch early Python errors (like the missing six.py module). >:-(
    lldb.stdout.setEncoding('utf8');
    lldb.stdout.on('data', (data: string) => {
        if (data.indexOf('Traceback') >= 0) {
            send_error_msg('Failed to launch LLDB: check debug console for error messages.', data);
        }
    });
}

function send_error_msg(slideout: string, message: string) {
    // Send this info as a console event
    let event = JSON.stringify({
        type: 'event', seq: 0, event: 'output', body:
        { category: 'console', output: message }
    });
    process.stdout.write('\r\nContent-Length: ' + event.length.toString() + '\r\n\r\n');
    process.stdout.write(event);
    // Also, fake out a response to 'initialize' message, which will be shown on a slide-out.
    let response = JSON.stringify({
        type: 'response', command: 'initialize', request_seq: 1, success: false, body:
        { error: { id: 0, format: slideout, showUser: true } }
    });
    process.stdout.write('Content-Length: ' + response.length.toString() + '\r\n\r\n');
    process.stdout.write(response);
}

main();
