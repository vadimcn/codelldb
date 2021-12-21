#!/bin/sh

echo "Hello, this is a wrapper!" 1>&2
echo "I will run the following commands: $@" 1>&2
exec $@
