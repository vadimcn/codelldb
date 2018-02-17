import sys

if 'linux' in sys.platform or 'darwin' in sys.platform:
    import resource
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)

    # Limits debuger's memory usage to 16GB to prevent runaway visualizers from killing the machine
    def enable():
        resource.setrlimit(resource.RLIMIT_AS, (16 * 1024**3, hard))

    def disable():
        resource.setrlimit(resource.RLIMIT_AS, (soft, hard))
else:
    def enable():
        pass
    def disable():
        pass
