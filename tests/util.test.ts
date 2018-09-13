import * as assert from 'assert';
import * as ver from '../extension/ver';
import * as util from '../extension/util';

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

        assert.equal(util.expandVariables('', expander), '');
        assert.equal(util.expandVariables('AAAA${echo:TEST}BBBB', expander), 'AAAATESTBBBB');
        assert.equal(util.expandVariables('AAAA${}${echo:FOO}BBBB${reverse:BAR}CCCC', expander),
            'AAAA${}FOOBBBBRABCCCC');
        assert.throws(() => util.expandVariables('sdfhksadjfh${hren:FOO}wqerqwer', expander));
    });

    test('mergeValues', async () => {
        assert.deepEqual(util.mergeValues(10, undefined), 10);
        assert.deepEqual(util.mergeValues(false, true), true);
        assert.deepEqual(util.mergeValues(10, 0), 0);
        assert.deepEqual(util.mergeValues("100", "200"), "200");
        assert.deepEqual(util.mergeValues(
            [1, 2], [3, 4]),
            [1, 2, 3, 4]);
        assert.deepEqual(util.mergeValues(
            { a: 1, b: 2, c: 3 }, { a: 10, d: 40 }),
            { a: 10, b: 2, c: 3, d: 40 });
    });

    test('Environment', async () => {
        let env = new util.Environment(true);
        env['Foo'] = '111';
        env['FOO'] = '222';
        assert.equal(env['Foo'], '222');
        assert.equal(env['FOO'], '222');
        assert.equal(env['fOO'], '222');
        env['foo'] = '333';
        assert.equal(env['Foo'], '333');
        assert.equal(env['FOO'], '333');
        assert.equal(env['fOO'], '333');

        env['Bar'] = '123';
        for (let key in env) {
            assert.ok(key == 'Foo' || key == 'Bar');
        }
    });
})
