var cp = require('child_process');
var process = require('process');

var lldb = cp.spawn('lldb', ['-b', '-O', 'script import adapter; adapter.run_stdio_session(3,4)'], {
    stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'],
    cwd: __dirname
});

// In case there are problems with launching...
lldb.on('error', (err) => {
    var message = 'Failed to launch LLDB: ' + err.message;
    // Send this info as a console event
    var event = JSON.stringify({
        type: 'event', seq: 0, event: 'output', body:
        { category: 'console', output: message }
    });
    process.stdout.write('Content-Length: ' + event.length.toString() + '\r\n\r\n');
    process.stdout.write(event);
    // Also, fake out a response to 'initialize' message, which will be shown on a slide-out.
    var response = JSON.stringify({
        type: 'response', command: 'initialize', request_seq: 1, success: false, body:
            { error: { id: 0, format: message, showUser: true } }
    });
    process.stdout.write('Content-Length: ' + response.length.toString() + '\r\n\r\n');
    process.stdout.write(response);
    process.exit(1);
});

process.stdin.pipe(lldb.stdio[3]);
lldb.stdio[4].pipe(process.stdout);

// When lldb exits, we exit too
lldb.on('exit', (code) => {
    process.exit(code);
});
