import { ExtensionContext, window, OutputChannel, Uri, extensions, env, ProgressLocation } from 'vscode';
import * as zip from 'yauzl';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { Writable } from 'stream';
import * as async from './novsc/async';

const MaxRedirects = 10;

let activeInstallation: Promise<boolean> = null;

export async function ensurePlatformPackage(context: ExtensionContext, output: OutputChannel, modal: boolean): Promise<boolean> {

    if (await async.fs.exists(path.join(context.extensionPath, 'platform.ok')))
        return true;

    // Just wait if installation is already in progress.
    if (activeInstallation != null)
        return activeInstallation;

    activeInstallation = doEnsurePlatformPackage(context, output, modal);
    let result = await activeInstallation;
    activeInstallation = null;
    return result;
}

async function doEnsurePlatformPackage(context: ExtensionContext, output: OutputChannel, modal: boolean): Promise<boolean> {

    let packageUrl = await getPlatformPackageUrl();
    output.appendLine(`Installing platform package from ${packageUrl}`);

    try {
        await window.withProgress(
            {
                location: ProgressLocation.Notification,
                cancellable: false,
                title: 'Acquiring CodeLLDB platform package'
            },
            async (progress) => {
                let downloadTarget: string;

                let lastPercentage = 0;
                let reportProgress = (downloaded: number, contentLength: number) => {
                    let percentage = Math.round(downloaded / contentLength * 100);
                    progress.report({
                        message: `${percentage}%`,
                        increment: percentage - lastPercentage
                    });
                    lastPercentage = percentage;
                };

                if (packageUrl.scheme == 'file') {
                    downloadTarget = packageUrl.fsPath;
                    // Simulate download
                    for (var i = 0; i <= 100; ++i) {
                        await async.sleep(10);
                        reportProgress(i, 100);
                    }
                } else {
                    downloadTarget = path.join(os.tmpdir(), 'vscode-lldb-full.vsix');
                    await download(packageUrl, downloadTarget, reportProgress);
                }

                progress.report({
                    message: 'installing',
                    increment: 100 - lastPercentage,
                });
                await installVsix(context, downloadTarget);
            }
        );
    } catch (err) {
        output.append(`Error: ${err}`);
        output.show();
        // Show error message, but don't block on it.
        window.showErrorMessage(
            `Platform package installation failed: ${err}.\n\n` +
            'You can try downloading the package manually.\n' +
            'Once done, use "Install from VSIX..." command to install.',
            { modal: modal },
            `Open download URL in a browser`
        ).then(choice => {
            if (choice != undefined)
                env.openExternal(packageUrl);
        });
        return false;
    }

    output.appendLine('Done')
    return true;
}

async function getPlatformPackageUrl(): Promise<Uri> {
    let pkg = extensions.getExtension('vadimcn.vscode-lldb').packageJSON;
    let pp = pkg.config.platformPackages;
    let id = `${os.arch()}-${os.platform()}`;
    let platformPackage = pp.platforms[id];
    if (platformPackage == undefined) {
        throw new Error(`This platform (${id}) is not suported.`);
    }
    return Uri.parse(pp.url.replace('${version}', pkg.version).replace('${platformPackage}', platformPackage));
}

async function download(srcUrl: Uri, destPath: string,
    progress?: (downloaded: number, contentLength?: number) => void) {

    for (let i = 0; i < MaxRedirects; ++i) {
        let response = await async.https.get(srcUrl.toString(true));
        if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
            srcUrl = Uri.parse(response.headers.location);
        } else {
            return new Promise(async (resolve, reject) => {
                if (response.statusCode < 200 || response.statusCode >= 300) {
                    reject(new Error(`HTTP status ${response.statusCode} : ${response.statusMessage}`));
                }
                if (response.headers['content-type'] != 'application/octet-stream') {
                    reject(new Error('HTTP response does not contain an octet stream'));
                } else {
                    let stm = fs.createWriteStream(destPath);
                    let pipeStm = response.pipe(stm);
                    if (progress) {
                        let contentLength = response.headers['content-length'] ? Number.parseInt(response.headers['content-length']) : null;
                        let downloaded = 0;
                        response.on('data', (chunk) => {
                            downloaded += chunk.length;
                            progress(downloaded, contentLength);
                        })
                    }
                    pipeStm.on('finish', resolve);
                    pipeStm.on('error', reject);
                    response.on('error', reject);
                }
            });
        }
    }
}

async function installVsix(context: ExtensionContext, vsixPath: string) {
    let destDir = context.extensionPath;
    await extractZip(vsixPath, async (entry) => {
        if (!entry.fileName.startsWith('extension/'))
            return null; // Skip metadata files.
        if (entry.fileName.endsWith('/platform.ok'))
            return null; // Skip success indicator, we'll create it at the end.

        let destPath = path.join(destDir, entry.fileName.substr(10));
        await ensureDirectory(path.dirname(destPath));
        let stream = fs.createWriteStream(destPath);
        stream.on('finish', () => {
            let attrs = (entry.externalFileAttributes >> 16) & 0o7777;
            fs.chmod(destPath, attrs, (err) => { });
        });
        return stream;
    });
    await async.fs.writeFile(path.join(destDir, 'platform.ok'), '');
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
