#!/bin/bash
exec 3<>/dev/tcp/127.0.0.1/$1; tty >&3; read <&3
