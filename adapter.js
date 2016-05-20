var cp = require('child_process');
var process = require('process');

var lldb = cp.spawn('lldb', ['-b', '-O', 'script import adapter; adapter.run_stdio_session(3,4)'], {
    stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'],
    cwd: __dirname
});
process.stdin.pipe(lldb.stdio[3]);
lldb.stdio[4].pipe(process.stdout);

// When lldb exits, we exit too
lldb.on('exit', (code) => {
    process.exit(code);
});