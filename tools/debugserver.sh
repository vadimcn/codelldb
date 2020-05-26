SCRIPT_DIR=$(dirname $BASH_SOURCE)
export FOO=BAR
$SCRIPT_DIR/../build/lldb/bin/lldb-server platform --server --listen *:1234 --log-channels "lldb all"
