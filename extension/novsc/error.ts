
export class ErrorWithCause extends Error {
    cause?: Error;

    constructor(message: string, options?: { cause: Error }) {
        super(message);
        if (options?.cause) {
            this.cause = options.cause;
        }
    }
}

export function formatError(err: Error & { cause?: Error }): string {
    let result = '';
    while (err) {
        result += err.stack;
        if (!err?.cause)
            break;
        result += '\nCaused by: ';
        err = err.cause;
    }
    return result;
}
