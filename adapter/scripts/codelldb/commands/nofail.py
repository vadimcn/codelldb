class NoFailCommand:
    '''Runs another command ignoring failures.'''

    def __init__(self, debugger, internal_dict):
        pass

    def __call__(self, debugger, command, exe_ctx, result):
        debugger.HandleCommand(command)
