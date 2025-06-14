import { ExtensionContext, window, OutputChannel, Uri, extensions, env, ProgressLocation, commands } from 'vscode';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import * as async from './novsc/async';
import { isRosetta } from './novsc/adapter';

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
                let lastPercentage = 0;
                let reportProgress = (downloaded: number, contentLength: number) => {
                    let percentage = Math.round(downloaded / contentLength * 100);
                    progress.report({
                        message: `${percentage}%`,
                        increment: percentage - lastPercentage
                    });
                    lastPercentage = percentage;
                };

                let downloadTarget = path.join(os.tmpdir(), `codelldb-${process.pid}-${getRandomInt()}.vsix`);

                if (packageUrl.scheme != 'file') {
                    await download(packageUrl, downloadTarget, reportProgress);
                } else {
                    // Simulate download
                    await async.fs.copyFile(packageUrl.fsPath, downloadTarget);
                    for (var i = 0; i <= 100; ++i) {
                        await async.sleep(10);
                        reportProgress(i, 100);
                    }
                }

                progress.report({
                    message: 'installing',
                    increment: 100 - lastPercentage,
                });
                await commands.executeCommand('workbench.extensions.command.installFromVSIX', [Uri.file(downloadTarget)]);
                await async.fs.unlink(downloadTarget);
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
    let platform = os.platform();
    let arch = os.arch();
    if (await isRosetta()) {
        arch = 'arm64';
    }
    let id = `${platform}-${arch}`;
    let platformPackage = pp.platforms[id];
    if (platformPackage == undefined) {
        throw new Error(`This platform (${id}) is not suported.`);
    }
    return Uri.parse(pp.url.replace('${version}', pkg.version).replace('${platformPackage}', platformPackage));
}

async function download(srcUrl: Uri, destPath: string,
    progress?: (downloaded: number, contentLength?: number) => void) {

    let url = srcUrl.toString(true);
    for (let i = 0; i < MaxRedirects; ++i) {
        let response = await async.https.get(url);
        if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
            url = response.headers.location;
        } else {
            return new Promise((resolve, reject) => {
                if (response.statusCode < 200 || response.statusCode >= 300) {
                    reject(new Error(`HTTP status ${response.statusCode} : ${response.statusMessage}`));
                }
                if (response.headers['content-type'] != 'application/octet-stream') {
                    reject(new Error('HTTP response does not contain an octet stream'));
                } else {
                    let stm = fs.createWriteStream(destPath, { mode: 0o600 });
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

function getRandomInt(): number {
    return Math.floor(Math.random() * 1e10)
}
