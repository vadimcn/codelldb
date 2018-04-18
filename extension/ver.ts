export function cmp(ver_a:string, ver_b:string): number {
    let a_parts = ver_a.split('.');
    let b_parts = ver_b.split('.');
    for (var i = 0; i < Math.min(a_parts.length, b_parts.length); ++i) {
        let a = parseInt(a_parts[i]);
        let b = parseInt(b_parts[i]);
        if (a != b) {
            return a - b;
        }
    }
    return a_parts.length - b_parts.length;
}

export function lt(ver_a:string, ver_b:string): boolean {
    return cmp(ver_a, ver_b) < 0;
}

export function gt(ver_a:string, ver_b:string): boolean {
    return cmp(ver_a, ver_b) > 0;
}
