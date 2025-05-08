let process = require('node:process');
let fs = require('node:fs');
let refPerser = require('@apidevtools/json-schema-ref-parser');
let mergeAllOf = require('json-schema-merge-allof');

let input = process.argv[2];
let output = process.argv[3];

(async () => {
    let pkg = JSON.parse(fs.readFileSync(input));
    await refPerser.dereference(pkg);
    // VSCode doesn't like allOff in these schemas
    let ca = pkg.contributes.debuggers[0].configurationAttributes;
    ca.launch = mergeAllOf(ca.launch, { ignoreAdditionalProperties: true });
    ca.attach = mergeAllOf(ca.attach, { ignoreAdditionalProperties: true });
    fs.writeFileSync(output, JSON.stringify(pkg, null, 2));
})();
