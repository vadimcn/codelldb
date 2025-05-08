import * as assert from 'assert';
import * as ver from 'extension/novsc/ver';
import { expandVariables, mergeValues } from 'extension/novsc/expand';
import YAML from 'yaml';

suite('Versions', () => {
    test('comparisons', async () => {
        assert.ok(ver.lt('1.0.0', '2.0.0'));
        assert.ok(ver.lt('2.0.0', '2.2.0'));
        assert.ok(ver.lt('2.0', '2.0.0'));
        assert.ok(ver.lt('2.0.0', '2.2'));
        assert.ok(ver.lt('2.0.0', '100.0.0'));
    })
})

suite('Util', () => {
    test('expandVariables', async () => {
        function expander(type: string, key: string) {
            if (type == 'echo') return key;
            if (type == 'reverse') return key.split('').reverse().join('');
            throw new Error('Unknown ' + type + ' ' + key);
        }

        assert.equal(expandVariables('', expander), '');
        assert.equal(expandVariables('AAAA${echo:TEST}BBBB', expander), 'AAAATESTBBBB');
        assert.equal(expandVariables('AAAA${}${echo:FOO}BBBB${reverse:BAR}CCCC', expander),
            'AAAA${}FOOBBBBRABCCCC');
        assert.throws(() => expandVariables('sdfhksadjfh${hren:FOO}wqerqwer', expander));
    });

    test('mergeValues', async () => {
        assert.deepEqual(mergeValues(undefined, undefined), undefined);
        assert.deepEqual(mergeValues(10, undefined), 10);
        assert.deepEqual(mergeValues(undefined, 10), 10);
        assert.deepEqual(mergeValues(true, false), true);
        assert.deepEqual(mergeValues(0, 10), 0);
        assert.deepEqual(mergeValues("100", "200"), "100");
        assert.deepEqual(mergeValues(
            [1, 2], [3, 4]),
            [1, 2, 3, 4]);
        assert.deepEqual(mergeValues(
            [1, 2], [3, 4], true),
            [3, 4, 1, 2]);
        assert.deepEqual(mergeValues(
            { a: 1, b: 2, }, { b: 20, c: 40 }),
            { a: 1, b: 2, c: 40 });
    });
})

suite('Third party', () => {
    test('YAML', async () => {
        let obj = YAML.parse('{foo: "foo", bar: 5}');
        assert.equal(obj.foo, 'foo');
        assert.equal(obj.bar, 5);
    })
})
