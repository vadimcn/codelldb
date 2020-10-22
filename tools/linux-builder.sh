LLDB_ROOT=$1
docker run -it --rm --name=linux-builder \
    -v  $(realpath $(dirname $BASH_SOURCE)/..):/codelldb:ro \
    -v /tmp/.X11-unix:/tmp/.X11-unix:ro \
    -v /etc/localtime:/etc/localtime:ro \
    -v /etc/passwd:/etc/passwd:ro \
    -v /etc/group:/etc/group:ro \
    -v $LLDB_ROOT:/lldb:ro \
    vadimcn/linux-builder
