import * as assert from 'assert';
import * as vscode from 'vscode';

suite('Extension Test Suite', () => {
  suiteTeardown(() => {
    vscode.window.showInformationMessage('All tests done!');
  });

  test('Sample test', () => {
    assert.equal(1, 1);
  });
});
