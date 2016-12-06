:loop
    echo ----------------------
    lldb -b -O "script import adapter; adapter.run_tcp_server(multiple=False, extinfo='c:\\temp\\vscode-lldb-session')"
goto loop
