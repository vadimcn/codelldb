var cp = require('child_process');
var process = require('process');
var fs = require('fs')

// Point this to a file or a tty to see adapter's debug spew
var lldb_log = 'ignore'
var lldb_log = fs.openSync('/dev/pts/19', 'w');

var lldb = cp.spawn('lldb', ['-b', '-O', 'script import adapter; adapter.stdio(3,4)'], {
    // LLDB has readline attached to its stdin and sometimes spews debug messages to stdout,
    // all of which would interfere with debug session messaging.
    // Instead, we create two new pipes on fds 3 and 4 and connect them to launcher's stdin and stdout
    stdio: ['ignore', lldb_log, lldb_log, 'pipe', 'pipe'],
    cwd: __dirname
});
process.stdin.pipe(lldb.stdio[3]);
lldb.stdio[4].pipe(process.stdout);

// When lldb exits, we exit too
lldb.on('exit', (code) => {
    process.exit(code);
});
