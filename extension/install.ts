import { ExtensionContext, window, OutputChannel, Uri, extensions, env } from 'vscode';
import * as zip from 'yauzl';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { Writable } from 'stream';
import * as async from './novsc/async';

const MaxRedirects = 10;

export async function ensurePlatformPackage(context: ExtensionContext, output: OutputChannel): Promise<boolean> {

    if (await async.fs.exists(path.join(context.extensionPath, 'lldb/bin')))
        return true;

    output.show();
    output.appendLine('Acquiring platform package for CodeLLDB.');

    try {
        let packageUrl = await getPlatformPackageUrl();
        output.appendLine('Package is located at ' + packageUrl);

        let downloadTarget;
        if (packageUrl.scheme != 'file') {
            downloadTarget = path.join(os.tmpdir(), 'vscode-lldb-full.vsix');
            output.appendLine('Downloading...');
            try {
                let lastPercent = -100;
                await download(packageUrl, downloadTarget, (downloaded, contentLength) => {
                    let percent = Math.round(100 * downloaded / contentLength);
                    if (percent >= lastPercent + 5) {
                        output.appendLine(`Downloaded ${percent}%`);
                        lastPercent = percent;
                    }
                });
            } catch (err) {
                let choice = await window.showErrorMessage(
                    `Download of the platform package has failed:\n${err}.\n\n` +
                    `You can try downloading it manually.  Once downloaded, please use the "Install from VSIX..." command to install.`,
                    { modal: true },
                    'Open download URL in a browser'
                );
                if (choice != undefined) {
                    env.openExternal(packageUrl);
                }
                return false;
            }
        } else {
            downloadTarget = packageUrl.fsPath;
        }

        output.appendLine('Installing...')
        await installVsix(context, downloadTarget);
        output.appendLine('Done.')
        return true;
    } catch (err) {
        window.showErrorMessage(err.toString());
        return false;
    }
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
