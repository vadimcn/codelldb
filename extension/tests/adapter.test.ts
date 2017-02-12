'use strict'

import * as assert from 'assert';
import * as path from 'path';
import * as cp from 'child_process';
import * as fs from 'fs';
import { DebugClient } from 'vscode-debugadapter-testsupport';
import { DebugProtocol as dp } from 'vscode-debugprotocol';

var dc: DebugClient;

const projectDir = path.join(__dirname, '..', '..');

const debuggee = path.join(projectDir, 'out', 'tests', 'debuggee');
const debuggeeSource = path.normalize(path.join(projectDir, 'extension', 'tests', 'cpp', 'debuggee.cpp'));
const debuggeeHeader = path.normalize(path.join(projectDir, 'extension', 'tests', 'cpp', 'dir1', 'debuggee.h'));

const rusttypes = path.join(projectDir, 'out', 'tests', 'rusttypes');
const rusttypesSource = path.normalize(path.join(projectDir, 'extension', 'tests', 'rusttypes.rs'));

var port: number = null;
if (process.env.DEBUG_SERVER) {
    port = parseInt(process.env.DEBUG_SERVER)
    console.log('Debug server port:', port)
}

setup(() => {
    dc = new DebugClient('node', './out/tests/launcher.js', 'lldb');
    return dc.start(port);
});

teardown(() => dc.stop());

suite('Basic', () => {

    test('run program to the end', async () => {
        let terminatedAsync = dc.waitForEvent('terminated');
        await launch({ program: debuggee });
        await terminatedAsync;
    });

    test('run program with modified environment', async () => {
        let waitExitedAsync = dc.waitForEvent('exited');
        await launch({
            env: { 'FOO': 'bar' },
            program: debuggee,
            args: ['check_env', 'FOO', 'bar'],
        });
        let exitedEvent = await waitExitedAsync;
        // debuggee shall return 1 if env[argv[2]] == argv[3]
        assert.equal(exitedEvent.body.exitCode, 1);
    });

    test('stop on entry', async () => {
        let stopAsync = dc.waitForEvent('stopped');
        launch({ program: debuggee, stopOnEntry: true });
        let stopEvent = await stopAsync;
        if (process.platform.startsWith('win'))
            assert.equal(stopEvent.body.reason, 'exception');
        else
            assert.equal(stopEvent.body.reason, 'signal');
    });

    test('stop on a breakpoint', async () => {
        let bpLineSource = findMarker(debuggeeSource, '#BP1');
        let bpLineHeader = findMarker(debuggeeHeader, '#BPH1');
        let setBreakpointAsyncSource = setBreakpoint(debuggeeSource, bpLineSource);
        let setBreakpointAsyncHeader = setBreakpoint(debuggeeHeader, bpLineHeader);
        let waitForExitAsync = dc.waitForEvent('exited');
        let waitForStopAsync = dc.waitForEvent('stopped');

        await launch({ program: debuggee, args: ['header'] });
        await setBreakpointAsyncSource;
        await setBreakpointAsyncHeader;

        let stopEvent = await waitForStopAsync;
        await verifyLocation(stopEvent.body.threadId, debuggeeSource, bpLineSource);

        let waitForStopAsync2 = dc.waitForEvent('stopped');
        await dc.continueRequest({ threadId: 0 });
        let stopEvent2 = await waitForStopAsync2;
        await verifyLocation(stopEvent.body.threadId, debuggeeHeader, bpLineHeader);

        await dc.continueRequest({ threadId: 0 });
        await waitForExitAsync;
    });

    test('page stack', async () => {
        let bpLine = findMarker(debuggeeSource, '#BP2');
        let setBreakpointAsync = setBreakpoint(debuggeeSource, bpLine);
        let waitForStopAsync = dc.waitForEvent('stopped');
        await launch({ program: debuggee, args: ['deepstack'] });
        await setBreakpointAsync;
        let stoppedEvent = await waitForStopAsync;
        let response2 = await dc.stackTraceRequest({ threadId: stoppedEvent.body.threadId, startFrame: 20, levels: 10 });
        assert.equal(response2.body.stackFrames.length, 10)
        let response3 = await dc.scopesRequest({ frameId: response2.body.stackFrames[0].id });
        let response4 = await dc.variablesRequest({ variablesReference: response3.body.scopes[0].variablesReference });
        assert.equal(response4.body.variables[0].name, 'levelsToGo');
        assert.equal(response4.body.variables[0].value, '20');
    });

    test('variables', async () => {
        let bpLine = findMarker(debuggeeSource, '#BP3');
        let setBreakpointAsync = setBreakpoint(debuggeeSource, bpLine);
        let waitForStopAsync = dc.waitForEvent('stopped');
        await launch({ program: debuggee, args: ['vars'] });
        await setBreakpointAsync;
        let stoppedEvent = await waitForStopAsync;
        await verifyLocation(stoppedEvent.body.threadId, debuggeeSource, bpLine);
        let frames = await dc.stackTraceRequest({ threadId: stoppedEvent.body.threadId, startFrame: 0, levels: 1 });
        let scopes = await dc.scopesRequest({ frameId: frames.body.stackFrames[0].id });
        let localsRef = scopes.body.scopes[0].variablesReference;
        let locals = await readVariables(localsRef);
        //console.log('locals = ', locals);
        assertDictContains(locals, {
            'a': '30',
            'b': '40',
            'vec_int': 'size=10',
            's': 'Struct',
            'str1': '"The quick brown fox"',
        });

        let response1 = await dc.evaluateRequest({
            expression: 'vec_int', context: 'watch',
            frameId: frames.body.stackFrames[0].id
        });
        let v = await readVariables(response1.body.variablesReference);
        assertDictContains(v, { '[0]': 'size=5', '[9]': 'size=5' });
        // Check that vector has '[raw]' element.
        assert.ok(v.hasOwnProperty('[raw]'));

        // Set a variable and check that it has actually changed.
        await dc.send('setVariable', { variablesReference: localsRef, name: 'a', value: '100' });
        let locals2 = await readVariables(localsRef);
        assert.equal(locals2['a'], '100');
    });
});

suite('Attach tests - these may fail if your system has a locked-down ptrace() syscall', () => {
    // Many Linux systems restrict tracing to parent processes only, which lldb in this case isn't.
    // To allow unrestricted tracing run `echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope`.
    var debuggeeProc: cp.ChildProcess;

    suiteSetup(() => {
        debuggeeProc = cp.spawn(debuggee, ['inf_loop'], {});
    })

    suiteTeardown(() => {
        debuggeeProc.kill()
    })

    test('attach by pid', async () => {
        let asyncWaitStopped = dc.waitForEvent('stopped');
        let attachResp = await attach({ program: debuggee, pid: debuggeeProc.pid, stopOnEntry: true });
        assert(attachResp.success);
        await asyncWaitStopped;
    });

    test('attach by name - may fail if a copy of debuggee is already running', async () => {
        // To fix, try running `killall debuggee` (`taskkill /im debuggee.exe` on Windows)
        let asyncWaitStopped = dc.waitForEvent('stopped');
        let attachResp = await attach({ program: debuggee, stopOnEntry: true });
        assert(attachResp.success);
        await asyncWaitStopped;
        debuggeeProc.kill()
    });
})

suite('Rust data display tests - these require a Rust compiler', () => {
    test('basic', async () => {
        let bpLine = findMarker(rusttypesSource, '#BP1');
        let setBreakpointAsync = setBreakpoint(rusttypesSource, bpLine);
        let waitForStopAsync = dc.waitForEvent('stopped');
        await launch({ program: rusttypes, sourceLanguages: ['rust'] });
        await setBreakpointAsync;
        let stoppedEvent = await waitForStopAsync;
        await verifyLocation(stoppedEvent.body.threadId, rusttypesSource, bpLine);
        let frames = await dc.stackTraceRequest({ threadId: stoppedEvent.body.threadId, startFrame: 0, levels: 1 });
        let scopes = await dc.scopesRequest({ frameId: frames.body.stackFrames[0].id });
        let locals = await readVariables(scopes.body.scopes[0].variablesReference);
        //console.log('locals = ', locals);

        assertDictContains(locals, {
            'int': '17',
            'float': '3.1415926535000001',
            'tuple': '(1, "a", 42)',
            'reg_enum1': 'A',
            'reg_enum2': 'B(100, 200)',
            'reg_enum3': 'C{x:11.35, y:20.5}',
            'cstyle_enum1': 'A',
            'cstyle_enum2': 'B',
            'enc_enum1': 'Some("string")',
            'enc_enum2': 'Nothing',
            'tuple_struct': '(3, "xxx", -3)',
            'reg_struct': 'rusttypes::RegularStruct',
            'array': '(5) [1, 2, 3, 4, 5]',
            'slice': '(5) &[1, 2, 3, 4, 5]',
            'vec_int': '(10) vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]',
            'vec_str': '(5) vec!["111", "2222", "3333", "4444", "5555" ...]',
            'string': '"A String"',
            'str_slice': '"String slice"',
            'cstring': '"C String"',
            'osstring': '"OS String"',
            'class': 'rusttypes::PyKeywords'
        });

        let response1 = await dc.evaluateRequest({
            expression: 'vec_str', context: 'watch',
            frameId: frames.body.stackFrames[0].id
        });
        let vec_str = await readVariables(response1.body.variablesReference);
        //console.log(vec_str);
        assertDictContains(vec_str, { '[0]': '"111"', '[4]': '"5555"' });

        let response2 = await dc.evaluateRequest({
            expression: 'string', context: 'watch',
            frameId: frames.body.stackFrames[0].id
        });
        let rstring = await readVariables(response2.body.variablesReference);
        //console.log(rstring);
        assertDictContains(rstring, { '[0]': "'A'", '[7]': "'g'" });
    });
});

/////////////////////////////////////////////////////////////////////////////////////////////////

function findMarker(file: string, marker: string): number {
    let data = fs.readFileSync(file, 'utf8');
    let lines = data.split('\n');
    for (var i = 0; i < lines.length; ++i) {
        let pos = lines[i].indexOf(marker);
        if (pos >= 0) return i + 1;
    }
    throw Error('Marker not found');
}

async function launch(launchArgs: any): Promise<dp.LaunchResponse> {
    let waitForInit = dc.waitForEvent('initialized');
    await dc.initializeRequest()
    let attachResp = dc.launchRequest(launchArgs);
    await waitForInit;
    dc.configurationDoneRequest();
    return attachResp;
}

async function attach(attachArgs: any): Promise<dp.AttachResponse> {
    let waitForInit = dc.waitForEvent('initialized');
    await dc.initializeRequest()
    let attachResp = dc.attachRequest(attachArgs);
    await waitForInit;
    dc.configurationDoneRequest();
    return attachResp;
}

async function setBreakpoint(file: string, line: number): Promise<dp.SetBreakpointsResponse> {
    let waitStopAsync = dc.waitForEvent('stopped');
    await dc.waitForEvent('initialized');
    let breakpointResp = await dc.setBreakpointsRequest({
        source: { path: file },
        breakpoints: [{ line: line, column: 0 }],
    });
    let bp = breakpointResp.body.breakpoints[0];
    assert.ok(bp.verified);
    assert.equal(bp.line, line);
    return breakpointResp;
}

async function verifyLocation(threadId: number, file: string, line: number) {
    let stackResp = await dc.stackTraceRequest({ threadId: threadId });
    let topFrame = stackResp.body.stackFrames[0];
    assert.equal(topFrame.line, line);
}

async function readVariables(variablesReference: number): Promise<any> {
    let response = await dc.variablesRequest({ variablesReference: variablesReference });
    let vars: any = {};
    for (var v of response.body.variables) {
        vars[v.name] = v.value;
    }
    return vars;
}

function assertDictContains(dict: any, expected: any) {
    for (var key in expected) {
        assert.equal(dict[key], expected[key]);
    }
}
