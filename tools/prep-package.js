let process = require('node:process');
let fs = require('node:fs');
let jptr = require("json-pointer")

let package = JSON.parse(fs.readFileSync(process.argv[2]));

function expandRefs(obj) {
    if (obj != null && typeof (obj) == 'object') {
        if (obj instanceof Array) {
            for (let i = 0; i < obj.length; i++) {
                obj[i] = expandRefs(obj[i]);
            }
        } else {
            let ptr = obj['$ref'];
            if (ptr != undefined) {
                if (ptr.startsWith('#'))
                    ptr = ptr.substr(1);
                let referenced = jptr.get(package, ptr);
                for (let [key, value] of Object.entries(referenced)) {
                    obj[key] = value;
                }
                delete obj['$ref'];
            }

            for (let [key, value] of Object.entries(obj)) {
                obj[key] = expandRefs(value);
            }
        }
    }
    return obj;
}

expandRefs(package);

// delete package['dependencies']
// delete package['devDependencies']

fs.writeFileSync(process.argv[3], JSON.stringify(package, null, 2));
