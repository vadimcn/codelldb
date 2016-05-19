'use strict'

import * as assert from 'assert';
import * as path from 'path';
import * as cp from 'child_process';
import * as fs from 'fs';
import {DebugClient} from 'vscode-debugadapter-testsupport';

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

async function configure_and_launch(configure: () => Promise<any>, launchArgs: any) {
    return Promise.all([
        dc.waitForEvent('initialized')
            .then(() => configure())
            .then(() => dc.configurationDoneRequest()),
        dc.initializeRequest(),
        dc.launchRequest(launchArgs),
    ]);
}

setup(() => {
    dc = new DebugClient('node', './adapter.js', 'node');
    return dc.start(4711);
});

teardown(() => dc.stop());

test('should run program to the end', () => {
    return Promise.all([
        dc.configurationSequence(),
        dc.launch({ program: 'tests/out/debuggee' }),
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

test('should stop on a breakpoint', () => {
    let bp_line = findMarker(debuggeeSource, '#BP1');
    return Promise.all([
        configure_and_launch(() =>
            dc.setBreakpointsRequest({
                source: { path: debuggeeSource },
                breakpoints: [{ line: bp_line }]
            }),
            { program: 'tests/out/debuggee' }),
        dc.assertStoppedLocation('breakpoint', { path: debuggeeSource, line: bp_line })
    ]);
});
