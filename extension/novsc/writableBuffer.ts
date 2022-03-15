import * as stream from 'stream';

export class WritableBuffer extends stream.Writable {
    private buffer: Buffer;
    private size: number;
    private increment: number;

    static readonly DEFAULT_INCREMENT = 8 * 1024;
    constructor(initialSize: number = WritableBuffer.DEFAULT_INCREMENT, increment: number = WritableBuffer.DEFAULT_INCREMENT) {
        super({ decodeStrings: true })
        this.buffer = Buffer.allocUnsafe(initialSize);
        this.increment = increment;
        this.size = 0;
    }

    _write(chunk: Buffer, encoding: string, callback: () => void) {
        if (this.size + chunk.length > this.buffer.length) {
            let factor = Math.ceil((chunk.length - (this.buffer.length - this.size)) / this.increment);
            let newBuffer = Buffer.allocUnsafe(this.buffer.length + (this.increment * factor));
            this.buffer.copy(newBuffer, 0, 0, this.size);
            this.buffer = newBuffer;
        }
        chunk.copy(this.buffer, this.size, 0);
        this.size += chunk.length;
        callback();
    }

    public get length(): number {
        return this.size;
    }

    public get contents(): Buffer {
        return this.buffer.slice(0, this.size);
    }

    public head(length: number) {
        if (length > this.size)
            throw new Error('length > buffer size');
        return this.buffer.slice(0, length);
    }

    public remove(length: number) {
        if (length > this.size)
            throw new Error('length > buffer size');
        this.buffer.copy(this.buffer, 0, length, this.size);
        this.size -= length;
    }
}
