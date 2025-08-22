import { defineConfig } from '@vscode/test-cli';

export default defineConfig([
    {
        files: 'tests/extension_tests.js',
        mocha: {
            ui: 'tdd',
            timeout: 20000,
            require: 'source-map-support/register',
        }
    }
]);
