import {
    CancellationToken, debug, DebugSession, LanguageModelTextPart, LanguageModelTool,
    LanguageModelToolInvocationOptions, LanguageModelToolResult
} from 'vscode';

let debugSessions = new Map<string, DebugSession>();
debug.onDidStartDebugSession(session => debugSessions.set(session.id, session));
debug.onDidTerminateDebugSession(session => debugSessions.delete(session.id));


export class SessionInfoTool implements LanguageModelTool<any> {
    async invoke(options: LanguageModelToolInvocationOptions<any>, token: CancellationToken): Promise<LanguageModelToolResult> {
        let session = debug.activeDebugSession;
        if (session) {
            let statusResponse = await session.customRequest('evaluate', {
                expression: 'process status',
                context: '_command'
            });
            let processState = statusResponse.result;
            try {
                let backtraceResponse = await session.customRequest('evaluate', {
                    expression: 'thread backtrace',
                    context: '_command'
                });
                processState += '\n' + backtraceResponse.result;
            } catch (e) {
                // May fail if the process is running
            }
            let result = [new LanguageModelTextPart(
                `Current debug session:\n` +
                `Session id: ${session.id}\n` +
                `Name: ${session.name}\n` +
                `Workspace folder: ${session.workspaceFolder?.uri.toString()}\n` +
                `Process state:\n${processState}\n`
            )];
            return new LanguageModelToolResult(result);
        } else {
            return new LanguageModelToolResult([new LanguageModelTextPart('error: There is no current debug session.')]);
        }
    }
}

interface LLDBCommandArgs {
    session_id?: string;
    command: string;
}

export class LLDBCommandTool implements LanguageModelTool<LLDBCommandArgs> {
    async invoke(options: LanguageModelToolInvocationOptions<LLDBCommandArgs>, token: CancellationToken): Promise<LanguageModelToolResult> {
        let session;
        if (!options.input.session_id) {
            session = debug.activeDebugSession;
            if (!session) {
                return new LanguageModelToolResult([new LanguageModelTextPart('error: There is no current debug session.')]);
            }
        }
        else {
            session = debugSessions.get(options.input.session_id);
            if (!session) {
                return new LanguageModelToolResult([new LanguageModelTextPart('error: Invalid debug session id.')]);
            }
        }
        let response = await session.customRequest('evaluate', {
            expression: options.input.command,
            context: '_command'
        });
        return new LanguageModelToolResult([new LanguageModelTextPart(response.result)]);
    }
}
