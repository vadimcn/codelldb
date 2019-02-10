import * as zip from 'yauzl';
import * as https from 'https';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { IncomingMessage } from 'http';
import { ExtensionContext, window, OutputChannel, Uri, commands, extensions } from 'vscode';
import { Writable } from 'stream';
import { existsAsync } from './async';

const MaxRedirects = 10;

export async function ensurePlatformPackage(context: ExtensionContext, output: OutputChannel): Promise<boolean> {

    if (await existsAsync(path.join(context.extensionPath, 'lldb/bin')))
        return true;

    let choice = await window.showInformationMessage(
        'The selected debug adapter type requires installation of platform-specific files.',
        { modal: true },
        { title: 'Download and install automatically', id: 'auto' },
        { title: 'Open URL in a browser', id: 'manual' }
    );
    if (choice == undefined) {
        return false;
    }

    try {
        let packageUrl = await getPlatformPackageUrl();
        output.appendLine('Platform package is located at ' + packageUrl);
        if (choice.id == 'manual') {
            commands.executeCommand('vscode.open', Uri.parse(packageUrl));
            return false;
        }

        let vsixTmp = path.join(os.tmpdir(), 'vscode-lldb-full.vsix');
        output.show();
        output.appendLine('Downloading platform package...');
        try {
            try {
                let lastPercent = -100;
                await download(packageUrl, vsixTmp, (downloaded, contentLength) => {
                    let percent = Math.round(100 * downloaded / contentLength);
                    if (percent > lastPercent + 5) {
                        output.appendLine(`Downloaded ${percent}%`);
                        lastPercent = percent;
                    }
                });
            } catch (err) {
                let choice = await window.showErrorMessage(
                    `Download of the platform package has failed.\n` +
                    `${err}.\n\n` +
                    `You can try to download and install it manually.`,
                    { modal: true },
                    'Open URL in a browser'
                );
                if (choice != undefined) {
                    commands.executeCommand('vscode.open', Uri.parse(packageUrl));
                }
                return false;
            }
            output.appendLine('Download complete.');
            output.appendLine('Installing...')
        } catch (err) {
        }
        await installVsix(context, vsixTmp);
        output.appendLine('Done.')
        return true;
    } catch (err) {
        window.showErrorMessage(err.toString());
        return false;
    }
}

async function getPlatformPackageUrl(): Promise<string> {
    let pkg = extensions.getExtension('vadimcn.vscode-lldb').packageJSON;
    let pp = pkg.config.platformPackages;
    let platformPackage = pp.platforms[process.platform];
    if (platformPackage == undefined) {
        throw new Error('Current platform is not suported.');
    }
    return pp.url.replace('${version}', pkg.version).replace('${platformPackage}', platformPackage);
}

async function download(srcUrl: string, destPath: string,
    progress?: (downloaded: number, contentLength?: number) => void) {

    return new Promise(async (resolve, reject) => {
        let response;
        for (let i = 0; i < MaxRedirects; ++i) {
            response = await new Promise<IncomingMessage>(resolve => https.get(srcUrl, resolve));
            if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
                srcUrl = response.headers.location;
            } else {
                break;
            }
        }
        if (response.statusCode < 200 || response.statusCode >= 300) {
            reject(new Error(`HTTP status ${response.statusCode} : ${response.statusMessage}`));
        }
        if (response.headers['content-type'] != 'application/octet-stream') {
            reject(new Error('HTTP response does not contain an octet stream'));
        } else {
            let stm = fs.createWriteStream(destPath);
            response.pipe(stm);
            if (progress) {
                let contentLength = response.headers['content-length'] ? Number.parseInt(response.headers['content-length']) : null;
                let downloaded = 0;
                response.on('data', (chunk) => {
                    downloaded += chunk.length;
                    progress(downloaded, contentLength);
                })
            }
            response.on('end', resolve);
            response.on('error', reject);
        }
    });
}

async function installVsix(context: ExtensionContext, vsixPath: string) {
    let destDir = context.extensionPath;
    await extractZip(vsixPath, async (entry) => {
        if (entry.fileName.startsWith('extension/')) {
            let destPath = path.join(destDir, entry.fileName.substr(10));
            await ensureDirectory(path.dirname(destPath));
            let stream = fs.createWriteStream(destPath);
            stream.on('finish', () => {
                let attrs = (entry.externalFileAttributes >> 16) & 0o7777;
                fs.chmod(destPath, attrs, (err) => { });
            });
            return stream;
        }
        else {
            return null;
        }
    });
}

function extractZip(zipPath: string, callback: (entry: zip.Entry) => Promise<Writable> | null): Promise<void> {
    return new Promise((resolve, reject) =>
        zip.open(zipPath, { lazyEntries: true }, (err, zipfile) => {
            if (err) {
                reject(err);
            } else {
                zipfile.readEntry();
                zipfile.on('entry', (entry: zip.Entry) => {
                    callback(entry).then(outstream => {
                        if (outstream != null) {
                            zipfile.openReadStream(entry, (err, zipstream) => {
                                if (err) {
                                    reject(err);
                                } else {
                                    outstream.on('error', reject);
                                    zipstream.on('error', reject);
                                    zipstream.on('end', () => zipfile.readEntry());
                                    zipstream.pipe(outstream);
                                }
                            });
                        } else {
                            zipfile.readEntry();
                        }
                    });
                });
                zipfile.on('end', () => {
                    zipfile.close();
                    resolve();
                });
                zipfile.on('error', reject);
            }
        })
    );
}

async function ensureDirectory(dir: string) {
    let exists = await new Promise(resolve => fs.exists(dir, exists => resolve(exists)));
    if (!exists) {
        await ensureDirectory(path.dirname(dir));
        await new Promise((resolve, reject) => fs.mkdir(dir, err => {
            if (err) reject(err);
            else resolve();
        }));
    }
}
