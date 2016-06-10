'use strict'

import * as assert from 'assert';
import * as path from 'path';
import * as cp from 'child_process';
import * as fs from 'fs';
import {DebugClient} from 'vscode-debugadapter-testsupport';
import {DebugProtocol as dp} from 'vscode-debugprotocol';

var dc: DebugClient;
var debuggeeSource = path.join(process.cwd(), 'tests', 'debuggee.cpp');

function findMarker(file: string, marker: string): number {
    let data = fs.readFileSync(file, 'utf8');
    let lines = data.split('\n');
    for (var i = 0; i < lines.length; ++i) {
        let pos = lines[i].indexOf(marker);
        if (pos >= 0) return i + 1;
    }
    throw Error('Marker not found');
}

setup(() => {
    dc = new DebugClient('node', './adapter.js', 'lldb');
    return dc.start();
});

teardown(() => dc.stop());

test('should run program to the end', () => {
    return Promise.all([
        dc.configurationSequence(),
        dc.launch({ program: 'tests/out/debuggee' }),
        dc.waitForEvent('terminated')
    ]);
});

test('should run program with modified environment', () => {
    return Promise.all([
        dc.configurationSequence(),
        dc.assertOutput('stdout', 'FOO=bar'),
        dc.launch({ program: 'tests/out/debuggee', args: ['show_env', 'FOO'], env: { 'FOO': 'bar' } }),
        dc.waitForEvent('terminated')
    ]);
});

test('should stop on entry', () => {
    return Promise.all([
        dc.configurationSequence(),
        dc.launch({ program: 'tests/out/debuggee', stopOnEntry: true }),
        dc.assertStoppedLocation('signal', { path: null, line: null, column: null })
    ]);
});

async function attach(dc: DebugClient, attachArgs: dp.AttachRequestArguments): Promise<dp.AttachResponse> {
    let waitForInit = dc.waitForEvent('initialized');
    await dc.initializeRequest()
    let attachResp = dc.attachRequest(attachArgs);
    await waitForInit;
    dc.configurationDoneRequest();
    return attachResp;
}

test('should attach', async () => {
    let debuggee = cp.spawn('tests/out/debuggee', ['inf_loop'], {});
    let asyncWaitStopped = dc.waitForEvent('stopped');
    let attachResp = await attach(dc, { program: 'tests/out/debuggee', pid: debuggee.pid });
    assert(attachResp.success);
    await asyncWaitStopped;
});

test('should stop on a breakpoint', () => {
    let bp_line = findMarker(debuggeeSource, '#BP1');
    return dc.hitBreakpoint(
        { program: 'tests/out/debuggee' },
        { path: debuggeeSource, line: bp_line, verified: true });
});

test('should page stack', () => {
    let bp_line = findMarker(debuggeeSource, '#BP2');
    return Promise.all([
        dc.waitForEvent('stopped').then(async (response1) => {
            let response2 = await dc.stackTraceRequest({ threadId: response1.body.threadId, startFrame: 20, levels: 10 });
            assert(response2.body.stackFrames.length == 10)
            let response3 = await dc.scopesRequest({ frameId: response2.body.stackFrames[0].id });
            let response4 = await dc.variablesRequest({ variablesReference: response3.body.scopes[0].variablesReference });
            assert(response4.body.variables[0].name == 'levelsToGo');
            assert(response4.body.variables[0].value == '20');
        }),
        dc.hitBreakpoint(
            { program: 'tests/out/debuggee', args: ['deepstack'] },
            { path: debuggeeSource, line: bp_line, verified: true })
    ]);
});
