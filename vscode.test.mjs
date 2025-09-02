import { defineConfig } from '@vscode/test-cli';

export default defineConfig([
    {
        files: 'tests/extension_tests.js',
        workspaceFolder: '${CMAKE_SOURCE_DIR}/debuggee',
        mocha: {
            ui: 'tdd',
            timeout: 20000,
            require: 'source-map-support/register',
        }
    }
]);
